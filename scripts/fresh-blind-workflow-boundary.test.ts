import { describe, expect, test } from "bun:test";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  FRESH_BLIND_LEDGER_ISSUE,
  FRESH_BLIND_REPOSITORY,
  FRESH_BLIND_WORKFLOW_REF,
} from "./fresh-blind-workflow-boundary";

const boundary = {
  actions: "true",
  candidateSha: "a".repeat(40),
  ledgerIssue: FRESH_BLIND_LEDGER_ISSUE,
  repository: FRESH_BLIND_REPOSITORY,
  runAttempt: "1",
  runId: "123",
  workflowRef: FRESH_BLIND_WORKFLOW_REF,
  workflowSha: "b".repeat(40),
} as const;

describe("fresh blind official workflow boundary", () => {
  test("accepts only the official main workflow and exact run identity", () => {
    expect(() => assertFreshBlindWorkflowBoundary(boundary)).not.toThrow();
    for (const drift of [
      { actions: "false" },
      { repository: "fork/semath" },
      { ledgerIssue: "999" },
      { workflowRef: FRESH_BLIND_WORKFLOW_REF.replace("main", "feature") },
      { workflowSha: "short" },
      { runAttempt: "0" },
    ]) {
      expect(() =>
        assertFreshBlindWorkflowBoundary({ ...boundary, ...drift }),
      ).toThrow();
    }
  });

  test("requires the controlled Linux x86_64 runner", () => {
    expect(() => assertFreshBlindLinuxX64("linux", "x64")).not.toThrow();
    expect(() => assertFreshBlindLinuxX64("darwin", "arm64")).toThrow(
      "Linux x86_64",
    );
  });
});
