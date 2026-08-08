import { describe, expect, test } from "bun:test";
import type { QueryEnvelope } from "../../protocol/src/index";
import {
  enqueueWork,
  INITIAL_WORKER_LIFECYCLE,
  staleGenerationMessage,
  transitionWorkerLifecycle,
  type WorkRequest,
} from "./host-state";

function query(generation: number, priority?: WorkRequest["priority"]): WorkRequest {
  return {
    envelope: {
      analysisGeneration: generation,
      documentVersion: 1,
      epoch: "test:1",
      inventoryVersion: 1,
      protocolVersion: 1,
      query: { fileId: "main", kind: "diagnostics" },
    } satisfies QueryEnvelope,
    id: generation,
    kind: "query",
    ...(priority ? { priority } : {}),
  };
}

describe("pure Worker host policy", () => {
  test("orders mutations before cursor and background work without mutating input", () => {
    const original = [{ order: 0, request: query(1, "background") }];
    const withCursor = enqueueWork(original, query(2), 1);
    const withMutation = enqueueWork(
      withCursor,
      {
        changes: {
          analysisGeneration: 3,
          changes: [],
          epoch: "test:1",
          inventoryVersion: 2,
          protocolVersion: 1,
        },
        id: 3,
        kind: "change",
      },
      2,
    );

    expect(original.map((work) => work.request.id)).toEqual([1]);
    expect(withMutation.map((work) => work.request.id)).toEqual([3, 2, 1]);
  });

  test("accepts only current query generations", () => {
    expect(staleGenerationMessage(query(4), 4)).toBeUndefined();
    expect(staleGenerationMessage(query(5), 4)).toBeUndefined();
    expect(staleGenerationMessage(query(3), 4)).toBe(
      "Skipped generation 3; current generation is 4.",
    );
  });

  test("contains repeated crashes and resets the counter after success", () => {
    const one = transitionWorkerLifecycle(INITIAL_WORKER_LIFECYCLE, "failure");
    const recovered = transitionWorkerLifecycle(one, "success");
    const two = transitionWorkerLifecycle(one, "failure");
    const terminal = transitionWorkerLifecycle(two, "failure");

    expect(one).toEqual({ consecutiveFailures: 1, status: "active" });
    expect(recovered).toEqual(INITIAL_WORKER_LIFECYCLE);
    expect(terminal).toEqual({ consecutiveFailures: 3, status: "terminal" });
    expect(transitionWorkerLifecycle(terminal, "success")).toBe(terminal);
  });

  test("keeps generation and priority policy stable under long edit bursts", () => {
    let queue: ReturnType<typeof enqueueWork> = [];
    for (let generation = 0; generation < 1_000; generation++) {
      queue = enqueueWork(
        queue,
        query(generation, generation % 2 ? "background" : "cursor"),
        generation,
      );
      expect(staleGenerationMessage(query(generation), generation)).toBeUndefined();
      if (generation > 0) {
        expect(staleGenerationMessage(query(generation - 1), generation)).toContain(
          "Skipped generation",
        );
      }
    }
    expect(queue).toHaveLength(1_000);
    expect(queue.slice(0, 500).every((work) => work.request.priority !== "background")).toBe(
      true,
    );
  });
});
