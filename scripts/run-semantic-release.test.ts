import { describe, expect, test } from "bun:test";
import {
  assertCleanReleaseWorktree,
  assertCommittedWasmArtifactsMatchHead,
  semanticReleasePreflightSteps,
  semanticReleaseSteps,
} from "./run-semantic-release";

describe("semantic release orchestration", () => {
  test("verifies rebuilt WASM against HEAD before crossing the fresh blind boundary", () => {
    const steps = semanticReleaseSteps("/sealed/fixture.json", "/receipt.json");
    const labels = steps.map((step) =>
      step.kind === "command"
        ? [step.command, ...step.args].join(" ")
        : step.kind,
    );

    for (const label of [
      "sh scripts/build-wasm.sh",
      "sha256sum -c SHA256SUMS",
      "assert-committed-wasm-artifacts-match-head",
      "bun scripts/check-fresh-blind-fixture.ts",
      "assert-clean-release-worktree",
      "bun scripts/run-fresh-blind-release.ts",
    ]) {
      expect(labels).toContain(label);
    }
    expect(labels.indexOf("sh scripts/build-wasm.sh")).toBeLessThan(
      labels.indexOf("sha256sum -c SHA256SUMS"),
    );
    expect(labels.indexOf("sha256sum -c SHA256SUMS")).toBeLessThan(
      labels.indexOf("assert-committed-wasm-artifacts-match-head"),
    );
    expect(
      labels.indexOf("assert-committed-wasm-artifacts-match-head"),
    ).toBeLessThan(labels.indexOf("bun scripts/check-fresh-blind-fixture.ts"));
    expect(labels.indexOf("bun scripts/check-fresh-blind-fixture.ts")).toBeLessThan(
      labels.indexOf("assert-clean-release-worktree"),
    );
    expect(labels.indexOf("assert-clean-release-worktree")).toBeLessThan(
      labels.indexOf("bun scripts/run-fresh-blind-release.ts"),
    );
  });

  test("rejects any tracked release artifact changed by the build", () => {
    expect(() => assertCommittedWasmArtifactsMatchHead("\n")).not.toThrow();
    expect(() =>
      assertCommittedWasmArtifactsMatchHead(
        "lib/wasm/semath_wasm.js\nlib/wasm/semath_wasm_bg.wasm",
      ),
    ).toThrow(
      "release WASM artifacts differ from HEAD after the x86_64 build",
    );
    expect(() =>
      assertCommittedWasmArtifactsMatchHead("", "lib/wasm/debug.map"),
    ).toThrow("release WASM artifacts differ from HEAD after the x86_64 build");
  });

  test("requires the complete tracked and untracked worktree to remain clean", () => {
    expect(() => assertCleanReleaseWorktree("\n")).not.toThrow();
    expect(() =>
      assertCleanReleaseWorktree(" M packages/protocol/src/index.ts\n?? stray.txt"),
    ).toThrow("semantic release worktree changed before the fresh blind boundary");
  });

  test("stops a hosted release preflight at the clean one-shot boundary", () => {
    const steps = semanticReleasePreflightSteps("fixture.json", "receipt.json");
    const labels = steps.map((step) =>
      step.kind === "command"
        ? [step.command, ...step.args].join(" ")
        : step.kind,
    );

    expect(labels.at(-1)).toBe("assert-clean-release-worktree");
    expect(labels).not.toContain("bun scripts/run-fresh-blind-release.ts");
  });
});
