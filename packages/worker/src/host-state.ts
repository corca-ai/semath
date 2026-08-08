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
