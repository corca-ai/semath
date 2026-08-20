import { describe, expect, test } from "bun:test";
import { isApprovedReferenceFixturePath } from "./fresh-blind-evidence";

describe("fresh blind reference isolation", () => {
  test("opens only public development namespaces", () => {
    expect(
      [
        "fixtures/corpus/generated-v1.json",
        "fixtures/development/evidence-graded-hypotheses-v1.json",
        "fixtures/foundation/scientific-prose-v1.json",
        "fixtures/challenge/document-reasoning-development-v1.json",
        "fixtures/challenge/math-authoring-oracle-source-v2.json",
        "fixtures/challenge/semantic-continuity-v1.json",
      ].every(isApprovedReferenceFixturePath),
    ).toBe(true);
  });

  test("rejects every historical or prospective blind namespace before I/O", () => {
    expect(
      [
        "fixtures/challenge/document-reasoning-holdout-v1.json",
        "fixtures/challenge/document-reasoning-fresh-v028.json",
        "fixtures/challenge/document-reasoning-fresh-v037.json",
        "fixtures/receipts/v0.37.json",
        "./fixtures/challenge/document-reasoning-fresh-sentinel.json",
      ].some(isApprovedReferenceFixturePath),
    ).toBe(false);
  });
});
