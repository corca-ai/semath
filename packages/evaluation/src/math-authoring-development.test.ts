import { describe, expect, test } from "bun:test";
import type {
  MathAuthoringContext,
  MathInterpretationHypothesisInfo,
} from "../../protocol/src/index";
import {
  compareMathAuthoringContext,
  evaluateMathAuthoringDevelopment,
  MATH_AUTHORING_DEVELOPMENT_FACETS,
  mathAuthoringCrossDocumentEvidenceFiles,
  mathAuthoringExpectationCanonicalFailures,
  mathAuthoringExpectationFormulaRootFailures,
  mathAuthoringExpectedFacetPresent,
  mathAuthoringExpectationSourceFailures,
  mathAuthoringContextSafetyFailures,
  mathAuthoringExactRegressions,
  parseMathAuthoringContextExpectation,
  parseMathAuthoringDevelopmentFixture,
  parseMathAuthoringReportObservations,
  projectMathAuthoringContext,
  type StableMathAuthoringContext,
} from "./math-authoring-development";

describe("exact public MathAuthoringContext oracle", () => {
  test("rejects a mutation of every top-level authoring surface", () => {
    const baseline = context();
    const expected = projectMathAuthoringContext(baseline);
    const mutations: Readonly<Record<string, (value: MathAuthoringContext) => MathAuthoringContext>> = {
      approximation: (value) => {
        const { approximation: _removed, ...rest } = value;
        return rest;
      },
      claimEvidence: (value) => ({ ...value, claimEvidence: [] }),
      conditions: (value) => ({ ...value, conditions: [] }),
      conventionalCandidates: (value) => {
        const { conventionalCandidates: _removed, ...rest } = value;
        return rest;
      },
      disposition: (value) => ({ ...value, disposition: "partial" }),
      equationLinks: (value) => ({ ...value, equationLinks: [] }),
      formula: (value) => {
        const { formula: _removed, ...rest } = value;
        return rest;
      },
      lifecycle: (value) => ({
        ...value,
        lifecycle: { ...value.lifecycle, editable: false },
      }),
      interpretations: (value) => ({
        ...value,
        interpretations: { ...value.interpretations, hypotheses: [] },
      }),
      notationOccurrences: (value) => ({ ...value, notationOccurrences: [] }),
      requirements: (value) => ({ ...value, requirements: [] }),
      truncated: (value) => ({ ...value, truncated: true }),
    };

    for (const [surface, mutate] of Object.entries(mutations)) {
      expect(compareMathAuthoringContext(expected, mutate(baseline)), surface).not.toEqual([]);
    }
  });

  test("classifies nested evidence, authority, anchor, conflict, and lifecycle mutations", () => {
    const baseline = context();
    const expected = projectMathAuthoringContext(baseline);
    const hypothesis = baseline.interpretations.hypotheses[0]!;
    const evidence = hypothesis.evidence[0]!;
    const anchor = evidence.sourceAnchors[0]!;

    const tentative = replaceHypothesis(baseline, {
      ...hypothesis,
      support: "tentative",
    });
    expect(
      kinds(projectMathAuthoringContext(tentative), baseline),
    ).toContain("authority-escalation");
    expect(
      kinds(expected, replaceHypothesis(baseline, { ...hypothesis, support: "contradicted" })),
    ).toContain("false-conflict");
    expect(
      kinds(
        expected,
        replaceEvidence(baseline, {
          ...evidence,
          role: "contradicting",
        }),
      ),
    ).toContain("false-conflict");
    expect(
      kinds(
        expected,
        replaceEvidence(baseline, {
          ...evidence,
          provenance: "natural-language-extraction",
        }),
      ),
    ).toContain("mismatch");

    for (const sourceAnchor of [
      { ...anchor, documentVersion: 2 },
      { ...anchor, location: { ...anchor.location, fileId: "other" } },
      { ...anchor, location: { ...anchor.location, path: "other.tex" } },
      {
        ...anchor,
        location: {
          ...anchor.location,
          range: { ...anchor.location.range, endOffset: 5 },
        },
      },
      { ...anchor, scopePath: [1] },
    ]) {
      expect(
        kinds(
          expected,
          replaceEvidence(baseline, { ...evidence, sourceAnchors: [sourceAnchor] }),
        ),
      ).toContain("wrong-anchor");
    }
    expect(
      kinds(expected, {
        ...baseline,
        notationOccurrences: baseline.notationOccurrences.map((occurrence) => ({
          ...occurrence,
          occurrenceId: { ...occurrence.occurrenceId, fileId: "other" },
        })),
      }),
    ).toContain("wrong-anchor");

    const retractedAnchor = { ...anchor, lifecycle: "retracted" as const };
    expect(
      kinds(
        expected,
        replaceEvidence(baseline, { ...evidence, sourceAnchors: [retractedAnchor] }),
      ),
    ).toContain("unsafe-lifecycle");
    expect(
      kinds(expected, {
        ...baseline,
        lifecycle: { ...baseline.lifecycle, retracted: true },
      }),
    ).toContain("unsafe-lifecycle");
  });

  test("fails missing and unexpected collections, ordering, limits, and discriminator links", () => {
    const baseline = context();
    const expected = projectMathAuthoringContext(baseline);
    const hypothesis = baseline.interpretations.hypotheses[0]!;
    expect(
      kinds(expected, {
        ...baseline,
        requirements: [...baseline.requirements, baseline.requirements[0]!],
      }),
    ).toContain("unexpected");
    expect(
      kinds(expected, { ...baseline, equationLinks: [] }),
    ).toContain("missing");
    expect(
      kinds(
        expected,
        replaceHypothesis(baseline, { ...hypothesis, rank: 3 }),
      ),
    ).toContain("mismatch");
    expect(
      kinds(expected, {
        ...baseline,
        interpretations: {
          ...baseline.interpretations,
          analysisLimits: [{ evidence: [], kind: "candidate-set-capped" }],
          truncated: true,
        },
      }),
    ).not.toEqual([]);
    expect(
      kinds(
        expected,
        replaceHypothesis(baseline, {
          ...hypothesis,
          missingDiscriminatorIds: ["missing"],
        }),
      ),
    ).toContain("missing");
  });

  test("rejects malformed fixture contexts before comparison", () => {
    const stable = projectMathAuthoringContext(context());
    const missing = { ...stable } as Record<string, unknown>;
    delete missing.lifecycle;
    expect(() => parseMathAuthoringContextExpectation(missing, "probe.expected"))
      .toThrow("missing keys lifecycle");
    expect(() =>
      parseMathAuthoringContextExpectation(
        { ...stable, hiddenAuthority: true },
        "probe.expected",
      ),
    ).toThrow("unexpected keys hiddenAuthority");

    const rawIdentity = structuredClone(stable);
    const hypothesis = rawIdentity.interpretations.hypotheses[0]! as
      StableMathAuthoringContext["interpretations"]["hypotheses"][number] & {
        hypothesisId?: string;
      };
    hypothesis.hypothesisId = "opaque-engine-id";
    expect(() =>
      parseMathAuthoringContextExpectation(rawIdentity, "probe.expected"),
    ).toThrow("unexpected keys hypothesisId");

    const nested = structuredClone(stable) as StableMathAuthoringContext & {
      claimEvidence: Array<
        StableMathAuthoringContext["claimEvidence"][number] & {
          hidden?: boolean;
        }
      >;
    };
    nested.claimEvidence[0]!.hidden = true;
    expect(() => parseMathAuthoringContextExpectation(nested, "probe.expected"))
      .toThrow("unexpected keys hidden");

    const malformedReference = structuredClone(stable);
    const reference = malformedReference.interpretations.hypotheses[0]!
      .orderingReasons[0]!.evidence[0]! as unknown as {
        sourceAnchors: Array<{ lifecycle: string }>;
      };
    reference.sourceAnchors[0]!.lifecycle = "stale";
    expect(() =>
      parseMathAuthoringContextExpectation(malformedReference, "probe.expected")
    ).toThrow("expected current or retracted");

    const unexpectedReferenceKey = structuredClone(stable);
    const requirement = unexpectedReferenceKey.requirements[0]!;
    if (requirement.kind === "condition") throw new Error("expected evidence requirement");
    (requirement.evidence[0] as {
      hiddenIdentity?: string;
    }).hiddenIdentity = "opaque";
    expect(() =>
      parseMathAuthoringContextExpectation(unexpectedReferenceKey, "probe.expected")
    ).toThrow("unexpected keys hiddenIdentity");
  });

  test("oracles source anchors on requirement, ordering, and limit evidence references", () => {
    const baseline = context();
    const evidenceReference = baseline.interpretations.hypotheses[0]!
      .orderingReasons[0]!.evidence[0]!;
    const withLimit: MathAuthoringContext = {
      ...baseline,
      interpretations: {
        ...baseline.interpretations,
        analysisLimits: [{ evidence: [evidenceReference], kind: "candidate-set-capped" }],
        truncated: true,
      },
      truncated: true,
    };
    const expected = projectMathAuthoringContext(withLimit);
    const otherAnchor = {
      ...evidenceReference.sourceAnchors[0]!,
      location: {
        ...evidenceReference.sourceAnchors[0]!.location,
        fileId: "other",
      },
    };

    const changedOrdering = structuredClone(withLimit);
    (changedOrdering.interpretations.hypotheses[0]!.orderingReasons[0]!
      .evidence[0]!.sourceAnchors as typeof evidenceReference.sourceAnchors) = [otherAnchor];
    expect(kinds(expected, changedOrdering)).toContain("wrong-anchor");

    const changedRequirement = structuredClone(withLimit);
    const requirement = changedRequirement.requirements[0]!;
    if (requirement.kind === "condition") throw new Error("expected evidence requirement");
    (requirement.evidence[0]!
      .sourceAnchors as typeof evidenceReference.sourceAnchors) = [otherAnchor];
    expect(kinds(expected, changedRequirement)).toContain("wrong-anchor");

    const changedLimit = structuredClone(withLimit);
    (changedLimit.interpretations.analysisLimits[0]!.evidence[0]!
      .sourceAnchors as typeof evidenceReference.sourceAnchors) = [otherAnchor];
    expect(kinds(expected, changedLimit)).toContain("wrong-anchor");
  });

  test("rejects internally inconsistent stable keys, ordinals, anchors, and lifecycle", () => {
    const stable = projectMathAuthoringContext(context());

    const wrongKey = structuredClone(stable);
    (wrongKey.interpretations.hypotheses[0]!.key as { label: string }).label =
      "not the selected hypothesis";
    expect(() => parseMathAuthoringContextExpectation(wrongKey, "probe.expected"))
      .toThrow("does not match typed canonical key");

    const wrongGroup = structuredClone(stable);
    (wrongGroup.interpretations.hypotheses[0] as { hypothesisGroup: number })
      .hypothesisGroup = 4;
    expect(() => parseMathAuthoringContextExpectation(wrongGroup, "probe.expected"))
      .toThrow("hypothesisGroup: expected 0");

    const wrongAnchor = structuredClone(stable);
    (wrongAnchor.interpretations.hypotheses[0]!.evidence[0]!.sourceAnchors[0]!
      .location.range as { endOffset: number }).endOffset = 2;
    expect(() => parseMathAuthoringContextExpectation(wrongAnchor, "probe.expected"))
      .toThrow("missing range 0");

    const unsafeLifecycle = structuredClone(stable);
    (unsafeLifecycle.lifecycle as { generation: "authored" | "generated" })
      .generation = "generated";
    expect(() =>
      parseMathAuthoringContextExpectation(unsafeLifecycle, "probe.expected")
    ).toThrow("lifecycle.editable: unsafe lifecycle");
  });

  test("strictly extracts complete contexts from one public development report", () => {
    const raw = context();
    const expected = projectMathAuthoringContext(raw);
    expect(
      parseMathAuthoringReportObservations({
        results: [{ observations: [{ authoringContext: raw, caseId: "case" }] }],
      }),
    ).toEqual([{ authoringContext: expected, caseId: "case" }]);
    expect(() =>
      parseMathAuthoringReportObservations({
        results: [
          {
            observations: [
              { authoringContext: raw, caseId: "case" },
              { authoringContext: raw, caseId: "case" },
            ],
          },
        ],
      }),
    ).toThrow("public observation caseId: duplicate case");

    const malformedSupport = structuredClone(raw) as MathAuthoringContext;
    (malformedSupport.interpretations.hypotheses[0] as {
      support: string;
    }).support = "invented-authority";
    expect(() =>
      parseMathAuthoringReportObservations({
        results: [{ observations: [{ authoringContext: malformedSupport, caseId: "case" }] }],
      }),
    ).toThrow("expected explicit or derived or supported or tentative or contradicted");

    const malformedLifecycle = structuredClone(raw) as MathAuthoringContext;
    (malformedLifecycle.interpretations.hypotheses[0]!.evidence[0]!
      .sourceAnchors[0] as { lifecycle: string }).lifecycle = "stale";
    expect(() =>
      parseMathAuthoringReportObservations({
        results: [{ observations: [{ authoringContext: malformedLifecycle, caseId: "case" }] }],
      }),
    ).toThrow("expected current or retracted");
  });

  test("requires exactly one complete observation and expectation for every public case", () => {
    const raw = context();
    const expected = projectMathAuthoringContext(raw);
    expect(
      evaluateMathAuthoringDevelopment(
        [{ expected: { authoringContext: expected }, id: "case" }],
        [{ authoringContext: raw, caseId: "case" }],
      ),
    ).toMatchObject({ cases: 1, exactCases: 1, failures: [] });
    const incomplete = evaluateMathAuthoringDevelopment(
      [
        { expected: {}, id: "missing-expectation" },
        { expected: { authoringContext: expected }, id: "missing-observation" },
      ],
      [{ authoringContext: raw, caseId: "unexpected" }],
    );
    expect(incomplete.exactCases).toBe(0);
    expect(incomplete.findings.map((finding) => finding.kind)).toEqual([
      "unexpected",
      "missing",
      "missing",
      "missing",
    ]);
  });

  test("the reviewed baseline satisfies independent lifecycle and evidence invariants", () => {
    expect(mathAuthoringContextSafetyFailures(context())).toEqual([]);
  });

  test("never accepts a required fresh or public oracle as zero of zero", () => {
    expect(
      mathAuthoringExactRegressions(
        { cases: 0, exactCases: 0, failures: [] },
        12,
      ),
    ).toEqual([
      "authoring-context case count 0 differs from required 12",
      "exact authoring context 0/0; required 12/12",
    ]);
    expect(
      mathAuthoringExactRegressions(
        { cases: 12, exactCases: 11, failures: ["unsafe lifecycle"] },
        12,
      ),
    ).toEqual([
      "exact authoring context 11/12; required 12/12",
      "authoring-context safety: unsafe lifecycle",
    ]);
  });

  test("requires a bounded dedicated fixture with complete declared facet breadth", () => {
    const expected = projectMathAuthoringContext(context());
    const fixture = {
      cases: Array.from({ length: 12 }, (_, index) => ({
        expected,
        facets: index === 0 ? [...MATH_AUTHORING_DEVELOPMENT_FACETS] : [],
        id: `case-${index}`,
        probeId: `probe-${index}`,
      })),
      pairs: Array.from({ length: 6 }, (_, index) => ({
        id: `pair-${index}`,
        latexCaseId: `case-${index * 2}`,
        markdownCaseId: `case-${index * 2 + 1}`,
      })),
      review: {
        digest: "d".repeat(64),
        reviewedAt: "2026-08-20",
        reviewer: "independent-reviewer",
      },
      schemaVersion: 1,
      sourceFixture: "fixtures/challenge/document-reasoning-development-v1.json",
    };
    expect(parseMathAuthoringDevelopmentFixture(fixture).cases).toHaveLength(12);
    expect(() =>
      parseMathAuthoringDevelopmentFixture({ ...fixture, cases: [] }),
    ).toThrow("expected 12 to 20 independently reviewed cases");
    const missingFacet = structuredClone(fixture);
    missingFacet.cases[0]!.facets = missingFacet.cases[0]!.facets.filter(
      (facet) => facet !== "cap",
    );
    expect(() => parseMathAuthoringDevelopmentFixture(missingFacet)).toThrow(
      "missing cap coverage",
    );
    expect(() =>
      parseMathAuthoringDevelopmentFixture({
        ...fixture,
        pairs: fixture.pairs.slice(1),
      }),
    ).toThrow("every reviewed case must belong to one TeX/Markdown pair");
  });

  test("normalizes opaque identities to dense semantic groups", () => {
    const baseline = context();
    const changedOccurrence = {
      documentVersion: 1,
      fileId: "main",
      localId: 991,
    };
    const changedEntity = {
      ...baseline.notationOccurrences[0]!.entityId,
      anchor: changedOccurrence,
      componentId: "opaque-component-renumbered",
    };
    const changed = {
      ...baseline,
      claimEvidence: baseline.claimEvidence.map((claim) => ({
        ...claim,
        claimId: "opaque-claim-991",
        supportingClaimIds: [],
      })),
      conventionalCandidates: baseline.conventionalCandidates!.map((candidate) => ({
        ...candidate,
        candidateId: "opaque-candidate-991",
      })),
      equationLinks: baseline.equationLinks.map((link) => ({
        ...link,
        linkId: "opaque-link-991",
        sharedEntities: [changedEntity],
      })),
      interpretations: {
        ...baseline.interpretations,
        hypotheses: baseline.interpretations.hypotheses.map((hypothesis) => ({
          ...hypothesis,
          hypothesisId: "opaque-hypothesis-991",
          missingDiscriminatorIds: ["opaque-requirement-991"],
        })),
        missingDiscriminators: baseline.interpretations.missingDiscriminators.map(
          (requirement) => ({
            ...requirement,
            requirementId: "opaque-requirement-991",
          }),
        ),
      },
      notationOccurrences: baseline.notationOccurrences.map((occurrence) => ({
        ...occurrence,
        entityId: changedEntity,
        occurrenceId: changedOccurrence,
      })),
      requirements: baseline.requirements.map((requirement) => ({
        ...requirement,
        requirementId: "opaque-requirement-991",
      })),
    } satisfies MathAuthoringContext;
    expect(projectMathAuthoringContext(changed)).toEqual(
      projectMathAuthoringContext(baseline),
    );
  });

  test("canonicalizes multiset surfaces but preserves reviewed interpretation order", () => {
    const baseline = context();
    const secondCondition = {
      ...baseline.conditions[0]!,
      conditionId: "condition/positive",
      kind: "positive" as const,
      label: "x is positive",
    };
    const reordered = {
      ...baseline,
      conditions: [secondCondition, baseline.conditions[0]!],
    };
    const forward = {
      ...baseline,
      conditions: [baseline.conditions[0]!, secondCondition],
    };
    expect(projectMathAuthoringContext(reordered)).toEqual(
      projectMathAuthoringContext(forward),
    );
    expect(
      mathAuthoringExpectationCanonicalFailures(
        projectMathAuthoringContext(reordered),
      ),
    ).toEqual([]);

    const firstOccurrence = baseline.notationOccurrences[0]!;
    const notationOrderDiffersFromEntityOrder = {
      ...baseline,
      notationOccurrences: [
        {
          ...firstOccurrence,
          entityId: {
            ...firstOccurrence.entityId,
            anchor: {
              ...firstOccurrence.entityId.anchor,
              fileId: "a-entity",
            },
          },
          location: {
            fileId: "z-location",
            path: "z-location.tex",
            range: firstOccurrence.location.range,
          },
          occurrenceId: { ...firstOccurrence.occurrenceId, fileId: "z-location" },
          sourceNotation: "z",
        },
        {
          ...firstOccurrence,
          entityId: {
            ...firstOccurrence.entityId,
            anchor: {
              ...firstOccurrence.entityId.anchor,
              fileId: "z-entity",
            },
          },
          location: {
            fileId: "a-location",
            path: "a-location.tex",
            range: firstOccurrence.location.range,
          },
          occurrenceId: { ...firstOccurrence.occurrenceId, fileId: "a-location" },
          sourceNotation: "a",
        },
      ],
    } satisfies MathAuthoringContext;
    expect(
      mathAuthoringExpectationCanonicalFailures(
        projectMathAuthoringContext(notationOrderDiffersFromEntityOrder),
      ),
    ).toEqual([]);

    const declarationA = {
      evidence: [],
      kind: "declaration" as const,
      occurrenceId: { documentVersion: 1, fileId: "a", localId: 1 },
      requirementId: "requirement/a",
      symbol: "a",
    };
    const declarationZ = {
      ...declarationA,
      occurrenceId: { documentVersion: 1, fileId: "z", localId: 1 },
      requirementId: "requirement/z",
      symbol: "z",
    };
    const splitDeclarationCollections = {
      ...baseline,
      interpretations: {
        ...baseline.interpretations,
        hypotheses: [],
        missingDiscriminators: [declarationZ],
      },
      notationOccurrences: [],
      requirements: [declarationA],
    } satisfies MathAuthoringContext;
    expect(
      mathAuthoringExpectationCanonicalFailures(
        projectMathAuthoringContext(splitDeclarationCollections),
      ),
    ).toEqual([]);

    const hypothesis = baseline.interpretations.hypotheses[0]!;
    const orderChanged = replaceHypothesis(baseline, {
      ...hypothesis,
      orderingReasons: [...hypothesis.orderingReasons].reverse(),
    });
    expect(
      compareMathAuthoringContext(
        projectMathAuthoringContext(baseline),
        orderChanged,
      ),
    ).not.toEqual([]);
    expect(
      mathAuthoringExpectationCanonicalFailures(
        projectMathAuthoringContext(orderChanged),
      ),
    ).toEqual([]);

    const originalEvidence = hypothesis.evidence[0]!;
    const secondRange = { endOffset: 6, startOffset: 4 };
    const secondEvidence = {
      ...originalEvidence,
      evidence: {
        ...originalEvidence.evidence,
        ruleId: "test/second",
        sourceRanges: [secondRange],
      },
      sourceAnchors: [
        {
          ...originalEvidence.sourceAnchors[0]!,
          location: {
            ...originalEvidence.sourceAnchors[0]!.location,
            range: secondRange,
          },
        },
      ],
    };
    const evidenceBaseline = replaceHypothesis(baseline, {
      ...hypothesis,
      evidence: [originalEvidence, secondEvidence],
    });
    const evidenceReordered = replaceHypothesis(baseline, {
      ...hypothesis,
      evidence: [secondEvidence, originalEvidence],
    });
    expect(
      compareMathAuthoringContext(
        projectMathAuthoringContext(evidenceBaseline),
        evidenceReordered,
      ),
    ).not.toEqual([]);

    const reference = hypothesis.orderingReasons[0]!.evidence[0]!;
    const orderedReference = {
      evidence: {
        ...reference.evidence,
        sourceRanges: [...reference.evidence.sourceRanges, secondRange],
      },
      sourceAnchors: [
        ...reference.sourceAnchors,
        {
          ...reference.sourceAnchors[0]!,
          location: { ...reference.sourceAnchors[0]!.location, range: secondRange },
        },
      ],
    };
    const referenceBaseline = replaceHypothesis(baseline, {
      ...hypothesis,
      orderingReasons: [{ evidence: [orderedReference], kind: "explicit-evidence" }],
    });
    const referenceReordered = replaceHypothesis(baseline, {
      ...hypothesis,
      orderingReasons: [{
        evidence: [{
          evidence: {
            ...orderedReference.evidence,
            sourceRanges: [...orderedReference.evidence.sourceRanges].reverse(),
          },
          sourceAnchors: [...orderedReference.sourceAnchors].reverse(),
        }],
        kind: "explicit-evidence",
      }],
    });
    expect(
      compareMathAuthoringContext(
        projectMathAuthoringContext(referenceBaseline),
        referenceReordered,
      ),
    ).not.toEqual([]);

    const rawDisambiguation = rawRequirement(baseline.requirements[0]);
    const rawAlternativeA = {
      alternativeId: "alternative/a",
      evidence: rawDisambiguation.evidence,
      label: "a alternative",
      range: { endOffset: 2, startOffset: 1 },
    };
    const rawAlternativeZ = {
      ...rawAlternativeA,
      alternativeId: "alternative/z",
      label: "z alternative",
      range: { endOffset: 3, startOffset: 2 },
    };
    const rawRequirementWithAlternatives = {
      ...rawDisambiguation,
      alternatives: [rawAlternativeZ, rawAlternativeA],
    };
    const projectedAlternatives = projectMathAuthoringContext({
      ...baseline,
      interpretations: {
        ...baseline.interpretations,
        missingDiscriminators: [rawRequirementWithAlternatives],
      },
      requirements: [rawRequirementWithAlternatives],
    });
    expect(
      mathAuthoringExpectationCanonicalFailures(projectedAlternatives),
    ).toEqual([]);

    const missingWithCanonicalizedAlternatives = {
      ...rawRequirementWithAlternatives,
      alternatives: [rawAlternativeA, rawAlternativeZ],
      requirementId: "requirement/missing",
    };
    const regularWithCanonicalizedAlternatives = {
      ...rawRequirementWithAlternatives,
      requirementId: "requirement/regular",
    };
    const splitEquivalentRequirements = projectMathAuthoringContext({
      ...baseline,
      interpretations: {
        ...baseline.interpretations,
        hypotheses: [],
        missingDiscriminators: [missingWithCanonicalizedAlternatives],
      },
      requirements: [regularWithCanonicalizedAlternatives],
    });
    expect(
      mathAuthoringExpectationCanonicalFailures(splitEquivalentRequirements),
    ).toEqual([]);

    const disambiguation = expectedRequirement(projectedAlternatives.requirements[0]);
    const alternativeA = {
      alternativeGroup: 1,
      evidence: disambiguation.evidence,
      label: "a alternative",
      range: { endOffset: 2, startOffset: 1 },
    };
    const alternativeZ = {
      alternativeGroup: 0,
      evidence: disambiguation.evidence,
      label: "z alternative",
      range: { endOffset: 3, startOffset: 2 },
    };
    const relabeledAlternatives = {
      ...projectMathAuthoringContext(baseline),
      requirements: [{
        ...disambiguation,
        alternatives: [alternativeZ, alternativeA],
      }],
    };
    expect(
      mathAuthoringExpectationCanonicalFailures(relabeledAlternatives),
    ).toContainEqual(expect.objectContaining({
      kind: "mismatch",
      path: "authoringContext.requirements[0].alternatives.alternativeGroup",
    }));
  });

  test("derives cross-document coverage only from reviewed selected evidence anchors", () => {
    const baseline = context();
    const sameFile = projectMathAuthoringContext(baseline);
    expect(mathAuthoringCrossDocumentEvidenceFiles(sameFile, "main")).toEqual([]);
    expect(
      mathAuthoringExpectedFacetPresent(
        sameFile,
        "main",
        "cross-document",
        false,
      ),
    ).toBeFalse();

    const hypothesis = baseline.interpretations.hypotheses[0]!;
    const evidence = hypothesis.evidence[0]!;
    const anchors = evidence.sourceAnchors.map((anchor) => ({
      ...anchor,
      location: {
        ...anchor.location,
        fileId: "reviewed-support",
        path: "reviewed-support.tex",
      },
    }));
    const crossDocument = projectMathAuthoringContext(
      replaceEvidence(baseline, { ...evidence, sourceAnchors: anchors }),
    );
    expect(mathAuthoringCrossDocumentEvidenceFiles(crossDocument, "main")).toEqual([
      "reviewed-support",
    ]);
    expect(
      mathAuthoringExpectedFacetPresent(
        crossDocument,
        "main",
        "cross-document",
        false,
      ),
    ).toBeTrue();
  });

  test("validates declared breadth facets against stable expectations", () => {
    const stable = projectMathAuthoringContext(context());
    for (const facet of [
      "approximation",
      "claim-evidence",
      "clean-incremental",
      "conditions",
      "conventional-candidates",
      "equation-links",
      "interpretations",
      "lifecycle",
      "notation",
      "requirements",
    ] as const) {
      expect(
        mathAuthoringExpectedFacetPresent(stable, "main", facet, false),
        facet,
      ).toBeTrue();
    }
    expect(mathAuthoringExpectedFacetPresent(stable, "main", "cap", false)).toBeFalse();
    expect(mathAuthoringExpectedFacetPresent(stable, "main", "generated", false)).toBeFalse();
    expect(
      mathAuthoringExpectedFacetPresent(
        stable,
        "main",
        "retraction-transition",
        false,
      ),
    ).toBeFalse();
    expect(
      mathAuthoringExpectedFacetPresent(
        stable,
        "main",
        "retraction-transition",
        true,
      ),
    ).toBeFalse();
  });

  test("rejects stable anchors outside their selected source document", () => {
    const stable = projectMathAuthoringContext(context());
    const source = {
      content: " x=y ",
      documentVersion: 1,
      fileId: "main",
      path: "main.tex",
    };
    expect(mathAuthoringExpectationSourceFailures(stable, [source])).toEqual([]);
    expect(
      mathAuthoringExpectationSourceFailures(stable, [
        { ...source, content: "a" },
      ]).map((failure) => failure.kind),
    ).toContain("wrong-anchor");
    expect(
      mathAuthoringExpectationSourceFailures(stable, [
        { ...source, documentVersion: 2 },
      ]).map((failure) => failure.kind),
    ).toContain("wrong-anchor");
  });

  test("requires exact syntax roots and canonical stable multiset order", () => {
    const stable = projectMathAuthoringContext(context());
    const source = {
      content: " x=y ",
      documentVersion: 1,
      fileId: "main",
      mathRootContentRanges: [{ endOffset: 4, startOffset: 1 }],
      path: "main.tex",
    };
    expect(mathAuthoringExpectationFormulaRootFailures(stable, [source])).toEqual([]);
    expect(mathAuthoringExpectationCanonicalFailures(stable)).toEqual([]);

    const inner = structuredClone(stable);
    inner.formula!.location.range = { endOffset: 3, startOffset: 1 };
    inner.formula!.sourceNotation = "x=";
    expect(
      mathAuthoringExpectationFormulaRootFailures(inner, [source])[0],
    ).toMatchObject({
      kind: "wrong-anchor",
      path: "authoringContext.formula.location.range",
    });

    const wrongNotation = structuredClone(stable);
    wrongNotation.formula!.sourceNotation = "x = y";
    expect(
      mathAuthoringExpectationSourceFailures(wrongNotation, [source])[0],
    ).toMatchObject({
      kind: "wrong-anchor",
      path: "authoringContext.formula.sourceNotation",
    });

    const noncanonical = {
      ...stable,
      conditions: [
        { ...stable.conditions[0]!, conditionId: "z-condition" },
        { ...stable.conditions[0]!, conditionId: "a-condition" },
      ],
    };
    expect(
      mathAuthoringExpectationCanonicalFailures(noncanonical)[0],
    ).toMatchObject({ kind: "mismatch", path: "authoringContext.conditions" });
  });
});

function context(): MathAuthoringContext {
  const range = { endOffset: 4, startOffset: 1 };
  const evidence = {
    kind: "source-structure",
    ruleId: "test/explicit",
    sourceRanges: [range],
    strength: "hard",
  };
  const location = { fileId: "main", path: "main.tex", range };
  const occurrenceId = { documentVersion: 1, fileId: "main", localId: 1 };
  const entityId = {
    anchor: occurrenceId,
    componentId: "main",
    kind: "symbol",
    scopePath: [],
  };
  const formula = {
    documentVersion: 1,
    location,
    scopePath: [],
    sourceNotation: "x=y",
  };
  const condition = {
    conditionId: "condition/nonzero",
    evidence: [evidence],
    kind: "nonzero" as const,
    label: "x is nonzero",
    status: "verified" as const,
    subjects: ["x"],
  };
  const relation = {
    conditions: [],
    description: "A reviewed relation.",
    evidence: [evidence],
    range,
    relationId: "test:law",
    roles: [{ label: "Value", role: "value", symbol: "x" }],
    title: "Test law",
  };
  const evidenceReference = {
    evidence,
    sourceAnchors: [
      {
        documentVersion: 1,
        generation: "authored" as const,
        lifecycle: "current" as const,
        location,
        scopePath: [],
      },
    ],
  };
  const requirement = {
    alternatives: [],
    evidence: [evidenceReference],
    kind: "disambiguation" as const,
    requirementId: "meaning/disambiguation/1-3",
  };
  const hypothesis: MathInterpretationHypothesisInfo = {
    bindings: [],
    conditions: [condition],
    documentVersion: 1,
    evidence: [
      {
        evidence,
        provenance: "explicit-declaration",
        role: "supporting",
        sourceAnchors: [
          {
            documentVersion: 1,
            generation: "authored",
            lifecycle: "current",
            location,
            scopePath: [],
          },
        ],
      },
    ],
    formula,
    hypothesisId: "source/test",
    kind: "source-meaning",
    label: "Explicit source meaning",
    location,
    missingDiscriminatorIds: [requirement.requirementId],
    orderingReasons: [
      { evidence: [evidenceReference], kind: "explicit-evidence" },
      { evidence: [], kind: "stable-source-order" },
    ],
    range,
    rank: 0,
    relation,
    scopePath: [],
    support: "explicit",
  };
  return {
    approximation: {
      evidence: [evidence],
      exactness: "approximate",
      relatedFactIds: ["fact/1"],
      relationRange: range,
    },
    claimEvidence: [
      {
        claim: location,
        claimId: "claim/1",
        evidence: [evidence],
        modality: "asserted",
        polarity: "positive",
        strengthCeiling: "asserted",
        supportingClaimIds: [],
        supportingFormulas: [formula],
      },
    ],
    conditions: [condition],
    conventionalCandidates: [
      {
        bindings: [],
        candidateId: "candidate/1",
        disposition: "conventional-candidate",
        evidence: [evidence],
        lawId: "test:law",
        packId: "test",
        packVersion: "1.0.0",
        relation,
        relevance: { evidence: [evidence], support: "supported" },
        requirements: [],
        title: "Candidate",
      },
    ],
    disposition: "established",
    equationLinks: [
      {
        evidence: [evidence],
        kind: "shared-entity",
        linkId: "link/1",
        sharedEntities: [entityId],
        source: formula,
        target: formula,
      },
    ],
    formula,
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    },
    interpretations: {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [hypothesis],
      missingDiscriminators: [requirement],
      truncated: false,
    },
    notationOccurrences: [
      {
        entityId,
        location,
        occurrenceId,
        scopePath: [],
        sourceNotation: "x",
      },
    ],
    requirements: [requirement],
    truncated: false,
  };
}

function replaceHypothesis(
  value: MathAuthoringContext,
  hypothesis: MathInterpretationHypothesisInfo,
): MathAuthoringContext {
  return {
    ...value,
    interpretations: { ...value.interpretations, hypotheses: [hypothesis] },
  };
}

function expectedRequirement(
  value: StableMathAuthoringContext["requirements"][number] | undefined,
): Extract<StableMathAuthoringContext["requirements"][number], {
  kind: "disambiguation";
}> {
  if (value?.kind !== "disambiguation") {
    throw new Error("expected a disambiguation requirement");
  }
  return value;
}

function rawRequirement(
  value: MathAuthoringContext["requirements"][number] | undefined,
): Extract<MathAuthoringContext["requirements"][number], {
  kind: "disambiguation";
}> {
  if (value?.kind !== "disambiguation") {
    throw new Error("expected a raw disambiguation requirement");
  }
  return value;
}

function replaceEvidence(
  value: MathAuthoringContext,
  evidence: MathInterpretationHypothesisInfo["evidence"][number],
): MathAuthoringContext {
  const hypothesis = value.interpretations.hypotheses[0]!;
  return replaceHypothesis(value, { ...hypothesis, evidence: [evidence] });
}

function kinds(
  expected: StableMathAuthoringContext,
  actual: MathAuthoringContext,
): readonly string[] {
  return compareMathAuthoringContext(expected, actual).map((finding) => finding.kind);
}
