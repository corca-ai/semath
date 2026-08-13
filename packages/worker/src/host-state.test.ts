import { describe, expect, test } from "bun:test";
import {
  type QueryEnvelope,
  SEMATH_PROTOCOL_VERSION,
} from "../../protocol/src/index";
import {
  advanceProjectFreshness,
  enqueueWork,
  INITIAL_WORKER_LIFECYCLE,
  staleGenerationMessage,
  staleProjectMessage,
  transitionWorkerLifecycle,
  type WorkRequest,
} from "./host-state";

function query(
  generation: number,
  priority?: WorkRequest["priority"],
): Extract<WorkRequest, { kind: "query" }> {
  return {
    envelope: {
      analysisGeneration: generation,
      documentVersion: 1,
      epoch: "test:1",
      inventoryVersion: 1,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
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
          protocolVersion: SEMATH_PROTOCOL_VERSION,
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
    expect(staleGenerationMessage(query(5), 4)).toBe(
      "Skipped future generation 5; current generation is 4.",
    );
    expect(staleGenerationMessage(query(3), 4)).toBe(
      "Skipped generation 3; current generation is 4.",
    );
    expect(
      staleProjectMessage(query(5), {
        analysisGeneration: 4,
        epoch: "test:1",
        inventoryVersion: 1,
      }),
    ).toBe("Skipped future generation 5; current generation is 4.");
  });

  test("does not let an older same-epoch reset move freshness backwards", () => {
    const current: Extract<WorkRequest, { kind: "reset" }> = {
      id: 1,
      kind: "reset",
      snapshot: {
        documents: [],
        epoch: "test:1",
        inventoryVersion: 4,
        projectId: "test",
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      },
    };
    const stale: Extract<WorkRequest, { kind: "reset" }> = {
      ...current,
      id: 2,
      snapshot: { ...current.snapshot, inventoryVersion: 3 },
    };

    const latest = advanceProjectFreshness(
      advanceProjectFreshness(undefined, current),
      stale,
    );

    expect(latest).toEqual({
      analysisGeneration: 0,
      epoch: "test:1",
      inventoryVersion: 4,
    });
  });

  test("rejects stale inventories and cross-project work before it reaches WASM", () => {
    const firstReset: Extract<WorkRequest, { kind: "reset" }> = {
      id: 1,
      kind: "reset",
      snapshot: {
        documents: [],
        epoch: "first:1",
        inventoryVersion: 4,
        projectId: "first",
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      },
    };
    const secondReset: Extract<WorkRequest, { kind: "reset" }> = {
      ...firstReset,
      id: 2,
      snapshot: {
        ...firstReset.snapshot,
        epoch: "second:1",
        inventoryVersion: 1,
        projectId: "second",
      },
    };
    const current = advanceProjectFreshness(
      advanceProjectFreshness(undefined, firstReset),
      secondReset,
    );

    expect(staleProjectMessage(firstReset, current)).toContain("Skipped epoch");
    expect(staleProjectMessage(secondReset, current)).toBeUndefined();
    expect(staleProjectMessage(query(0), current)).toContain("Skipped epoch");

    const currentQuery = query(0);
    currentQuery.envelope.epoch = "second:1";
    currentQuery.envelope.inventoryVersion = 1;
    expect(staleProjectMessage(currentQuery, current)).toBeUndefined();
    currentQuery.envelope.inventoryVersion = 2;
    expect(staleProjectMessage(currentQuery, current)).toContain("Skipped inventory");
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
