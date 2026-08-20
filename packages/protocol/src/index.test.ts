import { describe, expect, test } from "bun:test";
import {
  SEMATH_PROTOCOL_VERSION,
  canonicalMathInterpretationPreCapPayload,
  mathInterpretationPreCapSemanticKeyDigest,
  parseMathInterpretationCandidateCapInfo,
  type ConventionalCandidateInfo,
  type LawConditionInfo,
  type MathInterpretationEvidenceReferenceInfo,
  type MathInterpretationPreCapSemanticKey,
  type MathInterpretationSetInfo,
  type MathInterpretationRequirementInfo,
  type ProjectSnapshot,
  type SymbolInfo,
} from "./index";

function structuralPreCapKey(index: number): MathInterpretationPreCapSemanticKey {
  const range = { endOffset: index + 1, startOffset: index };
  const location = { fileId: "main", path: "main.tex", range };
  return {
    bindings: [],
    conditions: [],
    evidence: [
      {
        provenance: "typed-structure",
        role: "supporting",
        sourceAnchors: [
          {
            documentVersion: 1,
            generation: "authored",
            lifecycle: "current",
            location,
          },
        ],
      },
    ],
    formulaSource: {
      documentVersion: 1,
      generation: "authored",
      lifecycle: "current",
      location,
    },
    kind: "structural-alternative",
    label: `candidate ${index}`,
    relationId: null,
    support: "tentative",
  };
}

describe("protocol", () => {
  test("publishes the hard-cutover protocol version", () => {
    const snapshot: ProjectSnapshot = {
      documents: [],
      epoch: "project:1",
      inventoryVersion: 1,
      projectId: "project",
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    };
    expect(snapshot.protocolVersion).toBe(17);
  });

  test("keeps candidate cap metadata all-or-none at the wire boundary", () => {
    const boundary: MathInterpretationSetInfo = {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [],
      missingDiscriminators: [],
      truncated: false,
    };
    const capped: MathInterpretationSetInfo = {
      ...boundary,
      analysisLimits: [{ evidence: [], kind: "candidate-set-capped" }],
      candidateCap: {
        candidateCountBeforeCap: 17,
        preCapSemanticKeyDigest: "a".repeat(64),
      },
      truncated: true,
    };

    expect(JSON.stringify(boundary)).not.toContain("candidateCap");
    expect(capped.candidateCap?.candidateCountBeforeCap).toBe(17);
    expect(parseMathInterpretationCandidateCapInfo(capped.candidateCap!)).toEqual(
      capped.candidateCap!,
    );
    expect(() =>
      parseMathInterpretationCandidateCapInfo({
        candidateCountBeforeCap: 16,
        preCapSemanticKeyDigest: "a".repeat(64),
      }),
    ).toThrow();
    expect(() =>
      parseMathInterpretationCandidateCapInfo({
        candidateCountBeforeCap: 17,
        extra: true,
        preCapSemanticKeyDigest: "A".repeat(64),
      }),
    ).toThrow();
    expect(() =>
      parseMathInterpretationCandidateCapInfo({
        candidateCountBeforeCap: 0x1_0000_0000,
        preCapSemanticKeyDigest: "a".repeat(64),
      }),
    ).toThrow();
  });

  test("matches the protocol-17 pre-cap semantic digest vector", async () => {
    const candidates = Array.from({ length: 17 }, (_, index) =>
      structuralPreCapKey(index),
    );
    const reordered = [...candidates].reverse();
    const duplicated = [candidates[0]!, ...candidates];

    expect(canonicalMathInterpretationPreCapPayload(reordered)).toBe(
      canonicalMathInterpretationPreCapPayload(candidates),
    );
    expect(await mathInterpretationPreCapSemanticKeyDigest(candidates)).toBe(
      "da08f15f67c82e557e56b90af5aa7dd38db391b6f94c13ce982f43fb794646c4",
    );
    expect(await mathInterpretationPreCapSemanticKeyDigest(duplicated)).toBe(
      await mathInterpretationPreCapSemanticKeyDigest(candidates),
    );
  });

  test("hashes reviewable identity and provenance but ignores engine internals", async () => {
    const reviewable: MathInterpretationPreCapSemanticKey = {
      ...structuralPreCapKey(0),
      bindings: [{ parameter: "value", symbol: "x" }],
      conditions: [{ conditionId: "same-context", status: "verified" }],
    };
    const withInternals = {
      ...reviewable,
      bindings: reviewable.bindings.map((binding) => ({
        ...binding,
        constraint: { kind: "scalar" },
        proof: "derived",
      })),
      conditions: reviewable.conditions.map((condition) => ({
        ...condition,
        evidence: [{ ruleId: "internal" }],
        subjects: ["x"],
      })),
      evidence: reviewable.evidence.map((evidence) => ({
        ...evidence,
        ruleId: "internal",
        sourceAnchors: evidence.sourceAnchors.map((anchor) => ({
          ...anchor,
          scopePath: [9, 9],
        })),
      })),
      formulaSource: { ...reviewable.formulaSource, scopePath: [9, 9] },
      opaqueCandidateId: "internal/42",
    };
    const changedIdentity: MathInterpretationPreCapSemanticKey = {
      ...reviewable,
      label: "a different reviewed meaning",
    };
    const changedProvenance: MathInterpretationPreCapSemanticKey = {
      ...reviewable,
      evidence: reviewable.evidence.map((evidence) => ({
        ...evidence,
        provenance: "domain-context",
      })),
    };
    const baseline = await mathInterpretationPreCapSemanticKeyDigest([reviewable]);

    expect(await mathInterpretationPreCapSemanticKeyDigest([withInternals])).toBe(
      baseline,
    );
    expect(await mathInterpretationPreCapSemanticKeyDigest([changedIdentity])).not.toBe(
      baseline,
    );
    expect(await mathInterpretationPreCapSemanticKeyDigest([changedProvenance])).not.toBe(
      baseline,
    );
  });

  test("allows omitted empty role collections from the wire format", () => {
    const info: SymbolInfo = {
      definitions: [],
      diagnostics: [],
      location: {
        fileId: "main",
        path: "main.md",
        range: { endOffset: 2, startOffset: 1 },
      },
      occurrenceId: {
        documentVersion: 1,
        fileId: "main",
        localId: 0,
      },
      notation: [],
      sourceNotation: "x",
      shapes: [],
      symbol: "x",
      truncated: false,
    };
    expect(info.roles).toBeUndefined();
  });

  test("keeps operator requirements separate from display labels", () => {
    const condition: LawConditionInfo = {
      conditionId: "linear-map",
      evidence: [],
      kind: "operator-property",
      label: "The operator is linear.",
      operatorProperty: "linear",
      status: "required",
      subjects: ["A"],
    };
    expect(condition.operatorProperty).toBe("linear");
  });

  test("keeps conventional candidates structurally non-authoritative", () => {
    const candidate: ConventionalCandidateInfo = {
      bindings: [],
      candidateId: "conventional/linear-algebra/matrix-vector-product/1:5",
      disposition: "conventional-candidate",
      evidence: [],
      lawId: "matrix-vector-product",
      packId: "linear-algebra",
      packVersion: "1.4.0",
      relation: {
        conditions: [],
        description: "A matrix maps a vector.",
        evidence: [],
        range: { endOffset: 5, startOffset: 1 },
        relationId: "linear-algebra:matrix-vector-product",
        roles: [],
        title: "Matrix-vector product",
      },
      relevance: { evidence: [], support: "supported" },
      requirements: [],
      title: "Matrix-vector product",
    };
    expect(candidate.disposition).toBe("conventional-candidate");
  });

  test("keeps exact document identity on every interpretation evidence reference", () => {
    const range = { endOffset: 60, startOffset: 50 } as const;
    const references: readonly MathInterpretationEvidenceReferenceInfo[] = [
      {
        evidence: {
          kind: "attached-prose",
          ruleId: "english-respectively-definition",
          sourceRanges: [range, range],
          strength: "strong",
        },
        sourceAnchors: [
          {
            documentVersion: 3,
            generation: "authored",
            lifecycle: "current",
            location: { fileId: "roles-a", path: "roles-a.tex", range },
            scopePath: [0, 1],
          },
          {
            documentVersion: 4,
            generation: "authored",
            lifecycle: "current",
            location: { fileId: "roles-b", path: "roles-b.tex", range },
            scopePath: [0, 2],
          },
        ],
      },
    ];
    const requirement: MathInterpretationRequirementInfo = {
      condition: {
        conditionId: "compatible-shapes",
        evidence: references,
        kind: "shape-compatible",
        label: "The shapes are compatible.",
        status: "required",
        subjects: ["A", "x"],
      },
      kind: "condition",
      requirementId: "law/condition/compatible-shapes",
    };

    const serialized = JSON.parse(JSON.stringify(requirement)) as {
      condition: {
        evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      };
    };
    const reference = serialized.condition.evidence[0]!;
    expect(reference.sourceAnchors.map((anchor) => anchor.location.fileId)).toEqual([
      "roles-a",
      "roles-b",
    ]);
    expect(reference.sourceAnchors.map((anchor) => anchor.location.range)).toEqual(
      [...reference.evidence.sourceRanges],
    );
  });
});
