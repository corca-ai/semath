import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, test } from "bun:test";
import {
  finalizeFreshBlindReceipt,
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
    try {
      await expect(
        finalizeFreshBlindReceipt(join(directory, "receipt.json"), receipt("started")),
      ).rejects.toThrow("terminal status");
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
