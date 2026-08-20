import { describe, expect, test } from "bun:test";
import {
  assertSemanticReleaseStepPlan,
  SEMANTIC_RELEASE_STEPS,
  SEMANTIC_RELEASE_SPEND_STEPS,
} from "./run-semantic-release";

describe("semantic release orchestration", () => {
  test("keeps the fresh engine last after committed-WASM and identity checks", () => {
    expect(() => assertSemanticReleaseStepPlan()).not.toThrow();
    expect(SEMANTIC_RELEASE_SPEND_STEPS.at(-1)).toBe("fresh-engine");
    expect(SEMANTIC_RELEASE_STEPS.indexOf("wasm-build")).toBeLessThan(
      SEMANTIC_RELEASE_STEPS.indexOf("wasm-committed"),
    );
    expect(SEMANTIC_RELEASE_STEPS.indexOf("worktree-clean")).toBeLessThan(
      SEMANTIC_RELEASE_STEPS.length,
    );
    expect(SEMANTIC_RELEASE_SPEND_STEPS.slice(0, 2)).toEqual([
      "global-reservation",
      "reservation-identity",
    ]);
  });

  test("rejects plans that can spend before immutable artifact checks", () => {
    expect(() =>
      assertSemanticReleaseStepPlan(SEMANTIC_RELEASE_STEPS, [
        "fresh-engine",
        "global-reservation",
        "reservation-identity",
      ]),
    ).toThrow("fresh engine must be the final semantic release step");

    expect(() =>
      assertSemanticReleaseStepPlan([
        "wasm-committed",
        "wasm-build",
        "wasm-checksum",
        "worktree-clean",
        "fresh-static-validation",
        "identity-recheck",
        "native-build",
        "retained-package",
        "preblind-manifest",
      ]),
    ).toThrow("misordered");
  });
});
