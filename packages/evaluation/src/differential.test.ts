import { describe, expect, test } from "bun:test";
import {
  firstDifferentialFailure,
  planSemanticEditTrace,
  planSemanticLifecycleTraces,
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

  test("plans every evidence lifecycle with establishment, retraction, and recovery", () => {
    const traces = planSemanticLifecycleTraces(20);
    expect(traces).toEqual(planSemanticLifecycleTraces(20));
    expect(traces.map((trace) => trace.family)).toEqual([
      "declaration-retraction",
      "include-order",
      "macro-retraction",
      "malformed-recovery",
      "polarity-retraction",
      "typed-conflict-recovery",
    ]);
    for (const trace of traces) {
      expect(trace.initialExpectedDecision).toBe("established");
      expect(trace.stages.at(-1)?.expectedDecision).toBe("established");
      expect(trace.stages.some((stage) => stage.expectedDecision !== "established")).toBe(true);
    }
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
