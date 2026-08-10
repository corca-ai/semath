import { describe, expect, test } from "bun:test";
import {
  firstDifferentialFailure,
  planSemanticEditTrace,
  shrinkEditTrace,
} from "./differential";

describe("semantic differential planning", () => {
  test("plans deterministic conflict, retraction, malformed and topology edits", () => {
    const trace = planSemanticEditTrace(20);
    expect(trace).toEqual(planSemanticEditTrace(20));
    expect(trace.steps.map((step) => step.kind)).toEqual([
      "upsert", "upsert", "upsert", "upsert", "upsert", "upsert", "path-change", "remove",
    ]);
  });

  test("reports the first divergent stage and exact field", () => {
    const shared = { decision: { status: "established" }, range: { endOffset: 4, startOffset: 3 } };
    expect(
      firstDifferentialFailure([
        { name: "clean", value: shared },
        { name: "incremental", value: shared },
        { name: "wasm", value: { ...shared, range: { ...shared.range, endOffset: 5 } } },
      ]),
    ).toMatchObject({ path: "$.range.endOffset", stage: "wasm" });
  });

  test("shrinks a failing trace to the causally necessary edit", () => {
    const trace = planSemanticEditTrace(20);
    const shrunk = shrinkEditTrace(trace, (candidate) =>
      candidate.steps.some((step) => step.content?.includes("matrix")),
    );
    expect(shrunk.steps).toHaveLength(1);
    expect(shrunk.steps[0]?.content).toContain("matrix");
  });
});
