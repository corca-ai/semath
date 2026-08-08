import type {
  SemathWorkerPriority,
  SemathWorkerRequest,
} from "../../protocol/src/index";

export type WorkRequest = Exclude<
  SemathWorkerRequest,
  { kind: "cancel" | "dispose" }
>;

export interface PendingWork {
  order: number;
  request: WorkRequest;
}

export interface WorkerLifecycleState {
  consecutiveFailures: number;
  status: "active" | "disposed" | "terminal";
}

export interface ProjectFreshness {
  analysisGeneration: number;
  epoch: string;
  inventoryVersion: number;
}

export type WorkerLifecycleEvent = "dispose" | "failure" | "success";

export const INITIAL_WORKER_LIFECYCLE: WorkerLifecycleState = {
  consecutiveFailures: 0,
  status: "active",
};

const PRIORITY: Record<SemathWorkerPriority, number> = {
  mutation: 0,
  cursor: 1,
  background: 2,
};

export function enqueueWork(
  queue: readonly PendingWork[],
  request: WorkRequest,
  order: number,
): PendingWork[] {
  return [...queue, { order, request }].sort(
    (left, right) =>
      requestPriority(left.request) - requestPriority(right.request) ||
      left.order - right.order,
  );
}

export function requestPriority(request: WorkRequest): number {
  return PRIORITY[
    request.priority ?? (request.kind === "query" ? "cursor" : "mutation")
  ];
}

export function staleGenerationMessage(
  request: WorkRequest,
  latestGeneration: number,
): string | undefined {
  if (
    request.kind !== "query" ||
    request.envelope.analysisGeneration >= latestGeneration
  ) {
    return undefined;
  }
  return `Skipped generation ${request.envelope.analysisGeneration}; current generation is ${latestGeneration}.`;
}

export function advanceProjectFreshness(
  current: ProjectFreshness | undefined,
  request: WorkRequest,
): ProjectFreshness | undefined {
  if (request.kind === "reset") {
    return {
      analysisGeneration: 0,
      epoch: request.snapshot.epoch,
      inventoryVersion: request.snapshot.inventoryVersion,
    };
  }
  if (request.kind !== "change") return current;
  if (current && current.epoch !== request.changes.epoch) return current;
  return {
    analysisGeneration: Math.max(
      current?.analysisGeneration ?? 0,
      request.changes.analysisGeneration,
    ),
    epoch: request.changes.epoch,
    inventoryVersion: Math.max(
      current?.inventoryVersion ?? 0,
      request.changes.inventoryVersion,
    ),
  };
}

export function staleProjectMessage(
  request: WorkRequest,
  latest: ProjectFreshness | undefined,
): string | undefined {
  if (!latest) return undefined;
  const requested = requestFreshness(request);
  if (requested.epoch !== latest.epoch) {
    return `Skipped epoch ${requested.epoch}; current epoch is ${latest.epoch}.`;
  }
  // Changes are ordered mutations, so earlier envelopes must still run before
  // the latest one. Queries and resets, by contrast, can be fenced eagerly.
  if (request.kind === "change") return undefined;
  if (requested.inventoryVersion !== latest.inventoryVersion) {
    return `Skipped inventory ${requested.inventoryVersion}; current inventory is ${latest.inventoryVersion}.`;
  }
  if (requested.analysisGeneration < latest.analysisGeneration) {
    return `Skipped generation ${requested.analysisGeneration}; current generation is ${latest.analysisGeneration}.`;
  }
  return undefined;
}

function requestFreshness(request: WorkRequest): ProjectFreshness {
  if (request.kind === "reset") {
    return {
      analysisGeneration: 0,
      epoch: request.snapshot.epoch,
      inventoryVersion: request.snapshot.inventoryVersion,
    };
  }
  const envelope = request.kind === "change" ? request.changes : request.envelope;
  return {
    analysisGeneration: envelope.analysisGeneration,
    epoch: envelope.epoch,
    inventoryVersion: envelope.inventoryVersion,
  };
}

export function transitionWorkerLifecycle(
  state: WorkerLifecycleState,
  event: WorkerLifecycleEvent,
  failureLimit = 3,
): WorkerLifecycleState {
  if (state.status !== "active") return state;
  if (event === "dispose") return { ...state, status: "disposed" };
  if (event === "success") return INITIAL_WORKER_LIFECYCLE;
  const consecutiveFailures = state.consecutiveFailures + 1;
  return {
    consecutiveFailures,
    status: consecutiveFailures >= failureLimit ? "terminal" : "active",
  };
}
