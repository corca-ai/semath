import { describe, expect, test } from "bun:test";
import type {
  MathInterpretationHypothesisInfo,
  MathInterpretationSetInfo,
} from "../../protocol/src/index";
import {
  evidenceGradedBreadthFailures,
  summarizeEvidenceGradedHypotheses,
} from "./evidence-graded";

describe("evidence-graded hypothesis facets", () => {
  test("reports advisory facets separately without an aggregate score", () => {
    const interpretations = interpretationSet([
      hypothesis({
        kind: "scoped-domain",
        label: "Linear algebra",
        provenance: "domain-context",
      }),
      hypothesis({
        kind: "structural-alternative",
        label: "matrix",
        provenance: "natural-language-extraction",
        rank: 1,
      }),
    ]);
    expect(
      summarizeEvidenceGradedHypotheses([
        { caseId: "reviewed", interpretations },
      ]),
    ).toEqual({
      cases: 1,
      contradictionCases: 0,
      domainContextCases: 1,
      exactAnchorCases: 1,
      failures: [],
      missingDiscriminatorCases: 1,
      multipleHypothesisCases: 1,
      naturalLanguageCases: 1,
      openWorldCases: 1,
      orderingCases: 1,
      reviewedConventionCases: 0,
      supportingEvidenceCases: 1,
      withHypotheses: 1,
    });
  });

  test("rejects opaque authority, broken anchors, ordering, and discriminator links", () => {
    const baseline = hypothesis({
      kind: "reviewed-convention",
      label: "Convention",
      support: "explicit",
    });
    const unsafe: MathInterpretationHypothesisInfo = {
      ...baseline,
      location: { ...baseline.location, path: "" },
      missingDiscriminatorIds: ["missing"],
      orderingReasons: [{ evidence: [], kind: "typed-evidence" }],
    };
    const report = summarizeEvidenceGradedHypotheses([
      { caseId: "unsafe", interpretations: interpretationSet([unsafe]) },
      { caseId: "old" },
    ]);
    expect(report.failures).toEqual([
      "unsafe: incomplete interpretation evidence anchor",
      "unsafe: invalid evidence ordering",
      "unsafe: reviewed-convention acquired explicit authority",
      "unsafe: hypothesis references unknown discriminator missing",
      "old: missing protocol-16 interpretations",
    ]);
  });

  test("rejects range-only evidence on secondary interpretation surfaces", () => {
    const baseline = hypothesis({ kind: "typed-law", label: "Typed law" });
    const rangeOnly: MathInterpretationHypothesisInfo = {
      ...baseline,
      orderingReasons: baseline.orderingReasons.map((reason, index) =>
        index === 0
          ? {
              ...reason,
              evidence: reason.evidence.map((reference) => ({
                ...reference,
                sourceAnchors: [],
              })),
            }
          : reason,
      ),
    };
    const report = summarizeEvidenceGradedHypotheses([
      { caseId: "range-only", interpretations: interpretationSet([rangeOnly]) },
    ]);
    expect(report.exactAnchorCases).toBe(0);
    expect(report.failures).toEqual([
      "range-only: incomplete interpretation evidence anchor",
    ]);
  });

  test("rejects empty and zero-length interpretation evidence anchors", () => {
    const baseline = hypothesis({ kind: "typed-law", label: "Typed law" });
    const evidence = baseline.evidence[0]!;
    const sourceAnchor = evidence.sourceAnchors[0]!;
    const emptyEvidence: MathInterpretationHypothesisInfo = {
      ...baseline,
      evidence: [{
        ...evidence,
        evidence: { ...evidence.evidence, sourceRanges: [] },
        sourceAnchors: [],
      }],
    };
    const zeroLengthEvidence: MathInterpretationHypothesisInfo = {
      ...baseline,
      evidence: [{
        ...evidence,
        evidence: {
          ...evidence.evidence,
          sourceRanges: [{ endOffset: 1, startOffset: 1 }],
        },
        sourceAnchors: [{
          ...sourceAnchor,
          location: {
            ...sourceAnchor.location,
            range: { endOffset: 1, startOffset: 1 },
          },
        }],
      }],
    };
    const zeroLengthHypothesis: MathInterpretationHypothesisInfo = {
      ...baseline,
      location: {
        ...baseline.location,
        range: { endOffset: 1, startOffset: 1 },
      },
    };

    const report = summarizeEvidenceGradedHypotheses([
      { caseId: "empty", interpretations: interpretationSet([emptyEvidence]) },
      { caseId: "zero-evidence", interpretations: interpretationSet([zeroLengthEvidence]) },
      { caseId: "zero-hypothesis", interpretations: interpretationSet([zeroLengthHypothesis]) },
    ]);
    expect(report.exactAnchorCases).toBe(0);
    expect(report.failures).toEqual([
      "empty: incomplete interpretation evidence anchor",
      "zero-evidence: incomplete interpretation evidence anchor",
      "zero-hypothesis: incomplete interpretation evidence anchor",
    ]);
  });

  test("reports missing corpus facets without collapsing them into a score", () => {
    const report = summarizeEvidenceGradedHypotheses([
      {
        caseId: "single",
        interpretations: interpretationSet([
          hypothesis({ kind: "typed-law", label: "Typed law" }),
        ]),
      },
    ]);
    expect(evidenceGradedBreadthFailures(report)).toEqual([
      "evidence facets: missing contradiction coverage",
      "evidence facets: missing domain context coverage",
      "evidence facets: missing multiple hypothesis coverage",
      "evidence facets: missing natural-language provenance coverage",
      "evidence facets: missing reviewed convention coverage",
    ]);
  });
});

function interpretationSet(
  hypotheses: readonly MathInterpretationHypothesisInfo[],
): MathInterpretationSetInfo {
  return {
    analysisLimits: [],
    exhaustiveness: "bounded-open-world",
    hypotheses,
    missingDiscriminators: [
      {
        alternatives: [],
        evidence: [],
        kind: "disambiguation",
        requirementId: "meaning/structural-disambiguation/1-2",
      },
    ],
    truncated: false,
  };
}

function hypothesis(
  options: {
    kind: MathInterpretationHypothesisInfo["kind"];
    label: string;
    provenance?: MathInterpretationHypothesisInfo["evidence"][number]["provenance"];
    rank?: number;
    support?: MathInterpretationHypothesisInfo["support"];
  },
): MathInterpretationHypothesisInfo {
  return {
    bindings: [],
    conditions: [],
    documentVersion: 1,
    evidence: [
      {
        evidence: {
          kind: "source-structure",
          ruleId: "test/evidence",
          sourceRanges: [{ endOffset: 2, startOffset: 1 }],
          strength: "contextual",
        },
        provenance: options.provenance ?? "typed-structure",
        role: "supporting",
        sourceAnchors: [
          {
            documentVersion: 1,
            generation: "authored",
            lifecycle: "current",
            location: {
              fileId: "main",
              path: "main.tex",
              range: { endOffset: 2, startOffset: 1 },
            },
            scopePath: [],
          },
        ],
      },
    ],
    hypothesisId: `test/${options.label}`,
    kind: options.kind,
    label: options.label,
    location: {
      fileId: "main",
      path: "main.tex",
      range: { endOffset: 2, startOffset: 1 },
    },
    missingDiscriminatorIds: ["meaning/structural-disambiguation/1-2"],
    orderingReasons: [
      {
        evidence: [
          {
            evidence: {
              kind: "source-structure",
              ruleId: "test/evidence",
              sourceRanges: [{ endOffset: 2, startOffset: 1 }],
              strength: "contextual",
            },
            sourceAnchors: [{
              documentVersion: 1,
              generation: "authored",
              lifecycle: "current",
              location: {
                fileId: "main",
                path: "main.tex",
                range: { endOffset: 2, startOffset: 1 },
              },
              scopePath: [],
            }],
          },
        ],
        kind: "typed-evidence",
      },
      { evidence: [], kind: "stable-source-order" },
    ],
    range: { endOffset: 2, startOffset: 1 },
    rank: options.rank ?? 0,
    scopePath: [],
    support: options.support ?? "tentative",
  };
}
