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
      "domain-retraction",
      "formula-attachment-retraction",
      "declaration-retraction",
      "citation-retraction",
      "conditional-retraction",
      "include-order",
      "macro-retraction",
      "malformed-recovery",
      "polarity-retraction",
      "negation-retraction",
      "typed-conflict-recovery",
    ]);
    expect(
      traces.find((trace) => trace.family === "malformed-recovery")?.stages[0]?.queryNeedle,
    ).toBe("\\cap");
    for (const trace of traces) {
      expect(trace.stages.at(-1)?.expectedDecision).toBe(trace.initialExpectedDecision);
      expect(
        trace.stages.some(
          (stage) =>
            stage.expectedDecision !== trace.initialExpectedDecision ||
            JSON.stringify(stage.expectedDomains) !== JSON.stringify(trace.initialExpectedDomains),
        ),
      ).toBe(true);
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
