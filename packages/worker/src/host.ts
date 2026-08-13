import type { SemathWorkerRequest, SemathWorkerResponse } from "../../protocol/src/index";
import type { SemathWorkerEngine } from "./index";
import {
  advanceProjectFreshness,
  enqueueWork,
  INITIAL_WORKER_LIFECYCLE,
  staleProjectMessage,
  transitionWorkerLifecycle,
  type PendingWork,
  type ProjectFreshness,
  type WorkerLifecycleState,
  type WorkRequest,
} from "./host-state";

export interface SemathWorkerOperations {
  apply: SemathWorkerEngine["apply"];
  dispose: SemathWorkerEngine["dispose"];
  query: SemathWorkerEngine["query"];
  reset: SemathWorkerEngine["reset"];
}

/** Serial, reusable Worker host with cancellation and stale-project suppression. */
export class SemathWorkerHost {
  private cancelled = new Set<number>();
  private disposed = false;
  private draining = false;
  private enginePromise: Promise<SemathWorkerOperations> | undefined;
  private latestProject: ProjectFreshness | undefined;
  private lifecycle: WorkerLifecycleState = INITIAL_WORKER_LIFECYCLE;
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
    if (this.lifecycle.status === "terminal") {
      this.error(
        request.id,
        "runtime-failed",
        "Worker runtime stopped after three consecutive failures.",
        false,
      );
      return;
    }
    if (this.disposed) {
      this.error(request.id, "disposed", "Worker runtime has been disposed.", false);
      return;
    }
    this.latestProject = advanceProjectFreshness(this.latestProject, request);
    this.queue = enqueueWork(this.queue, request, this.order++);
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
        if (this.rejectPending(request)) continue;

        let engine: SemathWorkerOperations;
        try {
          engine = await this.getEngine();
        } catch (error) {
          this.enginePromise = undefined;
          this.lifecycle = transitionWorkerLifecycle(this.lifecycle, "failure");
          this.failure(request.id, "initialization-failed", error);
          continue;
        }

        if (this.rejectPending(request)) continue;

        try {
          const result = execute(engine, request);
          if (this.cancelled.delete(request.id)) {
            this.respond({ id: request.id, kind: "cancelled" });
          } else {
            this.lifecycle = transitionWorkerLifecycle(this.lifecycle, "success");
            this.respond({ id: request.id, kind: "result", result });
          }
        } catch (error) {
          engine.dispose();
          this.enginePromise = undefined;
          this.lifecycle = transitionWorkerLifecycle(this.lifecycle, "failure");
          this.failure(request.id, "engine-failed", error);
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

  private rejectPending(request: WorkRequest): boolean {
    if (this.cancelled.delete(request.id) || this.disposed) {
      this.respond({ id: request.id, kind: "cancelled" });
      return true;
    }
    if (this.lifecycle.status === "terminal") {
      this.error(
        request.id,
        "runtime-failed",
        "Worker runtime stopped after three consecutive failures.",
        false,
      );
      return true;
    }
    const stale = staleProjectMessage(request, this.latestProject);
    if (!stale) return false;
    this.error(request.id, "stale-generation", stale, true);
    return true;
  }

  private dispose(id: number): void {
    if (this.disposed) {
      this.respond({ id, kind: "disposed" });
      return;
    }
    this.disposed = true;
    this.lifecycle = transitionWorkerLifecycle(this.lifecycle, "dispose");
    for (const pending of this.queue) {
      this.respond({ id: pending.request.id, kind: "cancelled" });
    }
    this.queue = [];
    void this.enginePromise?.then((engine) => engine.dispose()).catch(() => undefined);
    this.respond({ id, kind: "disposed" });
  }

  private error(
    id: number,
    code:
      | "disposed"
      | "engine-failed"
      | "initialization-failed"
      | "runtime-failed"
      | "stale-generation",
    errorMessage: string,
    recoverable: boolean,
  ): void {
    this.respond({
      error: { code, message: errorMessage, recoverable },
      id,
      kind: "error",
    });
  }

  private failure(
    id: number,
    recoverableCode: "engine-failed" | "initialization-failed",
    cause: unknown,
  ): void {
    const terminal = this.lifecycle.status === "terminal";
    this.error(
      id,
      terminal ? "runtime-failed" : recoverableCode,
      terminal
        ? `Worker runtime stopped after three consecutive failures: ${message(cause)}`
        : message(cause),
      !terminal,
    );
  }
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
