import { describe, expect, test } from "bun:test";
import type {
  ChangeEnvelope,
  ProjectSnapshot,
  QueryEnvelope,
  SemathWorkerResponse,
} from "../../protocol/src/index";
import { SEMATH_PROTOCOL_VERSION } from "../../protocol/src/index";
import { SemathWorkerHost, type SemathWorkerOperations } from "./host";

const snapshot: ProjectSnapshot = {
  documents: [],
  epoch: "test:1",
  inventoryVersion: 1,
  projectId: "test",
  protocolVersion: SEMATH_PROTOCOL_VERSION,
};

function change(generation: number): ChangeEnvelope {
  return {
    analysisGeneration: generation,
    changes: [],
    epoch: "test:1",
    inventoryVersion: generation + 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

function query(generation: number): QueryEnvelope {
  return {
    analysisGeneration: generation,
    documentVersion: 1,
    epoch: "test:1",
    inventoryVersion: generation + 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: { fileId: "main", kind: "diagnostics" },
  };
}

function fakeEngine(log: string[], failQuery = false): SemathWorkerOperations {
  return {
    apply(envelope) {
      log.push(`change:${envelope.analysisGeneration}`);
      return { changedFileIds: [] };
    },
    dispose() {
      log.push("dispose");
    },
    query(envelope) {
      log.push(`query:${envelope.analysisGeneration}`);
      if (failQuery) throw new Error("worker crashed");
      return {} as ReturnType<SemathWorkerOperations["query"]>;
    },
    reset() {
      log.push("reset");
      return { changedFileIds: [] };
    },
  };
}

async function turn(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("SemathWorkerHost", () => {
  test("prioritizes mutations and skips stale or cancelled queries", async () => {
    const responses: SemathWorkerResponse[] = [];
    const log: string[] = [];
    const host = new SemathWorkerHost(async () => fakeEngine(log), (value) => responses.push(value));
    host.accept({
      envelope: query(1),
      id: 1,
      kind: "query",
      priority: "background",
    });
    host.accept({ changes: change(2), id: 2, kind: "change" });
    host.accept({ envelope: query(2), id: 3, kind: "query" });
    host.accept({ kind: "cancel", requestId: 3 });
    await turn();

    expect(log).toEqual(["change:2"]);
    expect(responses.map((response) => [response.id, response.kind])).toEqual([
      [2, "result"],
      [3, "cancelled"],
      [1, "error"],
    ]);
  });

  test("retries initialization and recreates an engine after a failed operation", async () => {
    const responses: SemathWorkerResponse[] = [];
    const log: string[] = [];
    let attempts = 0;
    const host = new SemathWorkerHost(async () => {
      attempts++;
      if (attempts === 1) throw new Error("init unavailable");
      return fakeEngine(log, attempts === 2);
    }, (value) => responses.push(value));

    host.accept({ id: 1, kind: "reset", snapshot });
    await turn();
    host.accept({ envelope: query(0), id: 2, kind: "query" });
    await turn();
    host.accept({ id: 3, kind: "reset", snapshot });
    await turn();

    expect(attempts).toBe(3);
    expect(log).toEqual(["query:0", "dispose", "reset"]);
    expect(responses.map((response) => response.kind)).toEqual(["error", "error", "result"]);
  });

  test("cancels queued work and frees the engine on disposal", async () => {
    const responses: SemathWorkerResponse[] = [];
    const log: string[] = [];
    const host = new SemathWorkerHost(async () => fakeEngine(log), (value) => responses.push(value));
    host.accept({ id: 1, kind: "reset", snapshot });
    host.accept({ id: 2, kind: "dispose" });
    await turn();

    expect(responses.map((response) => [response.id, response.kind])).toEqual([
      [1, "cancelled"],
      [2, "disposed"],
    ]);
    expect(log).toEqual([]);
  });

  test("reports a terminal failure instead of recreating engines forever", async () => {
    const responses: SemathWorkerResponse[] = [];
    let attempts = 0;
    const host = new SemathWorkerHost(async () => {
      attempts++;
      throw new Error("WASM unavailable");
    }, (value) => responses.push(value));

    for (const id of [1, 2, 3, 4]) {
      host.accept({ id, kind: "reset", snapshot });
      await turn();
    }

    expect(attempts).toBe(3);
    expect(
      responses.map((response) =>
        response.kind === "error"
          ? [response.error.code, response.error.recoverable]
          : [response.kind],
      ),
    ).toEqual([
      ["initialization-failed", true],
      ["initialization-failed", true],
      ["runtime-failed", false],
      ["runtime-failed", false],
    ]);
  });

  test("contains queued work from an old project during a rapid switch", async () => {
    const responses: SemathWorkerResponse[] = [];
    const log: string[] = [];
    const host = new SemathWorkerHost(async () => fakeEngine(log), (value) => responses.push(value));
    const nextSnapshot = {
      ...snapshot,
      epoch: "next:1",
      projectId: "next",
    };
    host.accept({ id: 1, kind: "reset", snapshot });
    host.accept({ envelope: query(0), id: 2, kind: "query" });
    host.accept({ id: 3, kind: "reset", snapshot: nextSnapshot });
    const nextQuery = query(0);
    nextQuery.epoch = nextSnapshot.epoch;
    nextQuery.inventoryVersion = nextSnapshot.inventoryVersion;
    host.accept({ envelope: nextQuery, id: 4, kind: "query" });
    await turn();

    expect(log).toEqual(["reset", "query:0"]);
    expect(responses.map((response) => [response.id, response.kind])).toEqual([
      [1, "error"],
      [3, "result"],
      [2, "error"],
      [4, "result"],
    ]);
  });
});
