import type {
  SemathWorkerPriority,
  SemathWorkerRequest,
  SemathWorkerResponse,
} from "../../protocol/src/index";
import type { SemathWorkerEngine } from "./index";

type WorkRequest = Exclude<SemathWorkerRequest, { kind: "cancel" | "dispose" }>;

interface PendingWork {
  order: number;
  request: WorkRequest;
}

export interface SemathWorkerOperations {
  apply: SemathWorkerEngine["apply"];
  dispose: SemathWorkerEngine["dispose"];
  query: SemathWorkerEngine["query"];
  reset: SemathWorkerEngine["reset"];
}

const PRIORITY: Record<SemathWorkerPriority, number> = {
  mutation: 0,
  cursor: 1,
  background: 2,
};

/** Serial, reusable Worker host with cancellation and stale-generation suppression. */
export class SemathWorkerHost {
  private cancelled = new Set<number>();
  private disposed = false;
  private draining = false;
  private enginePromise: Promise<SemathWorkerOperations> | undefined;
  private latestGeneration = 0;
  private order = 0;
  private queue: PendingWork[] = [];
  private scheduled = false;

  constructor(
    private readonly createEngine: () => Promise<SemathWorkerOperations>,
    private readonly respond: (response: SemathWorkerResponse) => void,
  ) {}

  accept(request: SemathWorkerRequest): void {
    if (request.kind === "cancel") {
      this.cancelled.add(request.requestId);
      return;
    }
    if (request.kind === "dispose") {
      this.dispose(request.id);
      return;
    }
    if (this.disposed) {
      this.error(request.id, "disposed", "Worker runtime has been disposed.", false);
      return;
    }
    if (request.kind === "change") {
      this.latestGeneration = Math.max(
        this.latestGeneration,
        request.changes.analysisGeneration,
      );
    }
    this.queue.push({ order: this.order++, request });
    this.queue.sort(
      (left, right) =>
        priority(left.request) - priority(right.request) || left.order - right.order,
    );
    this.scheduleDrain();
  }

  private scheduleDrain(): void {
    if (this.scheduled) return;
    this.scheduled = true;
    queueMicrotask(() => {
      this.scheduled = false;
      void this.drain();
    });
  }

  private async drain(): Promise<void> {
    if (this.draining) return;
    this.draining = true;
    try {
      while (!this.disposed && this.queue.length > 0) {
        const pending = this.queue.shift()!;
        const { request } = pending;
        if (this.cancelled.delete(request.id)) {
          this.respond({ id: request.id, kind: "cancelled" });
          continue;
        }
        if (
          request.kind === "query" &&
          request.envelope.analysisGeneration < this.latestGeneration
        ) {
          this.error(
            request.id,
            "stale-generation",
            `Skipped generation ${request.envelope.analysisGeneration}; current generation is ${this.latestGeneration}.`,
            true,
          );
          continue;
        }

        let engine: SemathWorkerOperations;
        try {
          engine = await this.getEngine();
        } catch (error) {
          this.enginePromise = undefined;
          this.error(request.id, "initialization-failed", message(error), true);
          continue;
        }

        try {
          const result = execute(engine, request);
          if (this.cancelled.delete(request.id)) {
            this.respond({ id: request.id, kind: "cancelled" });
          } else {
            this.respond({ id: request.id, kind: "result", result });
          }
        } catch (error) {
          engine.dispose();
          this.enginePromise = undefined;
          this.error(request.id, "engine-failed", message(error), true);
        }
      }
    } finally {
      this.draining = false;
    }
  }

  private getEngine(): Promise<SemathWorkerOperations> {
    this.enginePromise ??= this.createEngine();
    return this.enginePromise;
  }

  private dispose(id: number): void {
    if (this.disposed) {
      this.respond({ id, kind: "disposed" });
      return;
    }
    this.disposed = true;
    for (const pending of this.queue) {
      this.respond({ id: pending.request.id, kind: "cancelled" });
    }
    this.queue = [];
    void this.enginePromise?.then((engine) => engine.dispose()).catch(() => undefined);
    this.respond({ id, kind: "disposed" });
  }

  private error(
    id: number,
    code: "disposed" | "engine-failed" | "initialization-failed" | "stale-generation",
    errorMessage: string,
    recoverable: boolean,
  ): void {
    this.respond({
      error: { code, message: errorMessage, recoverable },
      id,
      kind: "error",
    });
  }
}

function priority(request: WorkRequest): number {
  return PRIORITY[request.priority ?? (request.kind === "query" ? "cursor" : "mutation")];
}

function execute(engine: SemathWorkerOperations, request: WorkRequest): unknown {
  switch (request.kind) {
    case "reset":
      return engine.reset(request.snapshot);
    case "change":
      return engine.apply(request.changes);
    case "query":
      return engine.query(request.envelope);
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
