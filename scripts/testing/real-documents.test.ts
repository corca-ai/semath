import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";

test("source collector rejects unsafe or drifting archives offline", () => {
  const result = spawnSync("python3", ["-B", "scripts/testing/test_real_documents.py"], { encoding: "utf8" });
  expect(result.stderr).toContain("OK");
  expect(result.status).toBe(0);
});
