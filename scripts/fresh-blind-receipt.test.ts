import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, test } from "bun:test";
import {
  finalizeFreshBlindReceipt,
  planFreshBlindReceiptTransition,
  reserveFreshBlindReceipt,
  type FreshBlindReleaseReceipt,
} from "./fresh-blind-receipt";

describe("fresh blind release receipt", () => {
  test("reserves exactly once and records the terminal result", async () => {
    const directory = await mkdtemp(join(tmpdir(), "semath-fresh-blind-"));
    const path = join(directory, "receipt.json");
    const started = receipt("started");
    try {
      await reserveFreshBlindReceipt(path, started);
      await expect(reserveFreshBlindReceipt(path, started)).rejects.toThrow();

      const completed: FreshBlindReleaseReceipt = {
        ...started,
        completedAt: "2026-08-12T01:01:00.000Z",
        result: { passed: 48 },
        status: "completed",
      };
      await finalizeFreshBlindReceipt(path, completed);
      expect(JSON.parse(await readFile(path, "utf8"))).toEqual(completed);
    } finally {
      await rm(directory, { recursive: true });
    }
  });

  test("rejects a non-terminal final receipt", async () => {
    const directory = await mkdtemp(join(tmpdir(), "semath-fresh-blind-"));
    const path = join(directory, "receipt.json");
    try {
      await reserveFreshBlindReceipt(path, receipt("started"));
      await expect(
        finalizeFreshBlindReceipt(path, receipt("started")),
      ).rejects.toThrow("terminal status");
    } finally {
      await rm(directory, { recursive: true });
    }
  });

  test("allows only an identity-preserving started-to-terminal transition", () => {
    const started = receipt("started");
    for (const status of [
      "completed",
      "safety-failed",
      "execution-error",
    ] as const) {
      const terminal: FreshBlindReleaseReceipt = {
        ...started,
        completedAt: "2026-08-12T01:01:00.000Z",
        status,
      };
      expect(planFreshBlindReceiptTransition(started, terminal)).toBe(terminal);
    }

    expect(() =>
      planFreshBlindReceiptTransition(receipt("completed"), {
        ...started,
        completedAt: "2026-08-12T01:01:00.000Z",
        status: "completed",
      }),
    ).toThrow("started receipt");
    expect(() =>
      planFreshBlindReceiptTransition(started, {
        ...started,
        completedAt: "2026-08-12T01:01:00.000Z",
        fixture: { ...started.fixture, seal: "f".repeat(64) },
        status: "completed",
      }),
    ).toThrow("same reserved execution");
  });

  test("atomically replaces started receipts and permanently rejects reruns", async () => {
    for (const status of [
      "completed",
      "safety-failed",
      "execution-error",
    ] as const) {
      const directory = await mkdtemp(join(tmpdir(), "semath-fresh-blind-"));
      const path = join(directory, "receipt.json");
      const started = receipt("started");
      const terminal: FreshBlindReleaseReceipt = {
        ...started,
        completedAt: "2026-08-12T01:01:00.000Z",
        status,
      };
      try {
        await reserveFreshBlindReceipt(path, started);
        await finalizeFreshBlindReceipt(path, terminal);
        expect(JSON.parse(await readFile(path, "utf8"))).toEqual(terminal);
        expect(await readdir(directory)).toEqual(["receipt.json"]);
        await expect(reserveFreshBlindReceipt(path, started)).rejects.toThrow();
        await expect(finalizeFreshBlindReceipt(path, terminal)).rejects.toThrow(
          "started receipt",
        );
      } finally {
        await rm(directory, { recursive: true });
      }
    }
  });

  test("does not create or mutate a receipt when transition policy rejects", async () => {
    const directory = await mkdtemp(join(tmpdir(), "semath-fresh-blind-"));
    const path = join(directory, "receipt.json");
    const started = receipt("started");
    try {
      await expect(
        finalizeFreshBlindReceipt(path, {
          ...started,
          completedAt: "2026-08-12T01:01:00.000Z",
          status: "completed",
        }),
      ).rejects.toThrow();
      expect(await readdir(directory)).toEqual([]);

      await reserveFreshBlindReceipt(path, started);
      await expect(
        finalizeFreshBlindReceipt(path, {
          ...started,
          completedAt: "2026-08-12T01:01:00.000Z",
          fixture: { ...started.fixture, id: "v0.99" },
          status: "completed",
        }),
      ).rejects.toThrow("same reserved execution");
      expect(JSON.parse(await readFile(path, "utf8"))).toEqual(started);
    } finally {
      await rm(directory, { recursive: true });
    }
  });
});

function receipt(
  status: FreshBlindReleaseReceipt["status"],
): FreshBlindReleaseReceipt {
  return {
    fixture: { id: "v0.28", seal: "a".repeat(64) },
    provenance: {
      nativeSha256: "b".repeat(64),
      semathCommit: "c".repeat(40),
      wasmSha256: "d".repeat(64),
      wasmtexCommit: "e".repeat(40),
    },
    schemaVersion: 1,
    startedAt: "2026-08-12T01:00:00.000Z",
    status,
  };
}
