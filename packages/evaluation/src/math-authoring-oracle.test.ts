import { describe, expect, test } from "bun:test";
import type { MathAuthoringContext } from "../../protocol/src/index";
import {
  compileMathAuthoringOracle,
  evaluateMathAuthoringOracle,
  isMathAuthoringRemovedContextSafelyAbsent,
  mathAuthoringDiagnosticArtifact,
  mathAuthoringDiagnosticArtifactPath,
  mathAuthoringExpectedObservationPlan,
  mathAuthoringOracleConstraintDigest,
  mathAuthoringOracleReviewDigest,
  mathAuthoringOracleReviewAttestationDigest,
  mathAuthoringOracleSourceDigest,
  parseMathAuthoringOracle,
  parseMathAuthoringOracleSource,
  type MathAuthoringOracle,
  type MathAuthoringOracleObservation,
  type MathAuthoringOracleReviewAttestation,
  type MathAuthoringOracleSource,
} from "./math-authoring-oracle";

describe("source-authored MathAuthoring oracle v2", () => {
  test("compiles the checked-in mixed-lifecycle reviewed oracle", async () => {
    const source = await Bun.file("fixtures/challenge/math-authoring-oracle-source-v2.json").json();
    const oracle = await Bun.file("fixtures/challenge/math-authoring-oracle-v2.json").json();
    const review = await Bun.file("fixtures/challenge/math-authoring-oracle-review-v2.json").json();
    const compiled = compileMathAuthoringOracle(source, oracle, review);
    expect(compiled.oracle.review).toEqual({
      attestationDigest: "3098aafd1b20618514c0011cc315c96b2d26f4121d4d574e51c28c6da3559607",
      author: "agent:/root",
      digest: "bf9d361ed7cd66f75b3fe9dc40a62d89d6add93ef3f06cccac5c56bff7c8adb2",
      reviewFixture: "fixtures/challenge/math-authoring-oracle-review-v2.json",
      reviewedAt: "2026-08-25",
      reviewer: "agent:/root/v042-main-reviewer",
    });
    expect(compiled.oracle.cases.find((item) => item.id === "open-world-cap-plus-one-tex")?.safety.disposition)
      .toBe("established");
    expect(compiled.oracle.cases.find((item) => item.id === "faraday-sign-conflict-tex")?.safety.lifecycle)
      .toEqual({
        capped: false,
        documentVersion: 1,
        editable: false,
        engineLimited: false,
        freshness: "current",
        generation: "authored",
        retracted: true,
      });
    expect(compiled.oracle.evidence["faraday-sign-conflict-tex-source-meaning-rejected-e1"])
      .toMatchObject({
        anchorStates: [
          {
            anchor: "faraday-sign-conflict-tex:rejected-formula.root",
            generation: "authored",
            lifecycle: "retracted",
          },
          {
            anchor: "faraday-sign-conflict-tex:refutation-current",
            generation: "authored",
            lifecycle: "current",
          },
        ],
        role: "contradicting",
      });
  });

  test("compiles unique UTF-16 named anchors and passes exact safety plus true pair parity", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    expect(Object.keys(compiled.anchors)).toHaveLength(40);
    expect(compiled.anchors["tex-0:formula"]!.location.range).toEqual({
      endOffset: 7,
      startOffset: 2,
    });

    const report = evaluateMathAuthoringOracle(compiled, observations(fixture));
    expect(report.safetyFailures).toEqual([]);
    expect(report.advisoryFindings).toEqual([]);
    expect(report.pairFailures).toEqual([]);
    expect(report.transitionFailures).toEqual([]);
    expect(report.diagnostic.artifactId).toBe(`sha256:${report.diagnostic.sha256}`);
  });

  test("derives versions from each file history and resolves nested needles", () => {
    const fixture = fixtureValue();
    const item = fixture.source.cases[0]!;
    const fileId = item.snapshots[0]!.documents[0]!.fileId;
    item.snapshots = [
      { dependencies: [], documents: [{ content: "A outer [\nx=y\n] relation.", fileId, path: fileId }], id: "first", mainFileId: fileId },
      { dependencies: [], documents: [{ content: "🧪 A revised outer [\nx=y\n] relation.", fileId, path: fileId }], id: "second", mainFileId: fileId },
      { dependencies: [], documents: [{ content: "🧪 A revised outer [\nx=y\n] relation.", fileId, path: fileId }], id: "unchanged", mainFileId: fileId },
    ];
    item.selections[0] = { anchor: "selection", id: "primary", snapshotId: "second" };
    item.namedNeedles = [
      { fileId, id: "outer", needle: "outer [\nx=y\n]", snapshotId: "second" },
      {
        fileId,
        id: "formula",
        needle: "\nx=y\n",
        snapshotId: "second",
        within: { anchor: "outer", needle: "[\nx=y\n]" },
      },
      { fileId, id: "selection", needle: "x=y", parentAnchor: "formula", snapshotId: "second" },
      { fileId, id: "formula-child", needle: "x=y", parentAnchor: "outer", snapshotId: "second" },
    ] as typeof item.namedNeedles;
    const markdown = fixture.source.cases[1]!;
    const markdownFileId = markdown.snapshots[0]!.mainFileId;
    markdown.snapshots = item.snapshots.map((snapshot) => ({
      dependencies: [],
      documents: snapshot.documents.map((document) => ({ ...document, fileId: markdownFileId, path: markdownFileId })),
      id: snapshot.id,
      mainFileId: markdownFileId,
    }));
    markdown.selections[0] = { anchor: "selection", id: "primary", snapshotId: "second" };
    markdown.namedNeedles = item.namedNeedles.map((needle) => ({ ...needle, fileId: markdownFileId }));
    fixture.oracle.cases[0]!.selectionId = "primary";
    finalize(fixture);
    const compiled = compileFixture(fixture);
    expect(compiled.anchors["tex-0:formula"]!.documentVersion).toBe(2);
    expect(compiled.anchors["tex-0:formula"]!.location.range.startOffset).toBe(
      "🧪 A revised outer [\nx=y\n] relation.".indexOf("\nx=y\n"),
    );
    expect(compiled.anchors["tex-0:formula-child"]!.location.range).toEqual(
      compiled.anchors["tex-0:selection"]!.location.range,
    );
    expect(compiled.source.cases[0]!.snapshots.map((snapshot) =>
      snapshot.documents[0]!.documentVersion
    )).toEqual([1, 2, 2]);
  });

  test("requires explicit acyclic dependency graphs with format parity and unique normalized paths", () => {
    const valid = dependencyFixture();
    expect(() => compileFixture(valid)).not.toThrow();

    const parity = dependencyFixture();
    parity.source.cases[1]!.snapshots[0]!.dependencies[0]!.sourceAnchor = "formula";
    expect(() => parseMathAuthoringOracleSource(parity.source)).toThrow("dependency topology mismatch");

    const nonNormalizedPath = dependencyFixture();
    nonNormalizedPath.source.cases[0]!.snapshots[0]!.documents[1]!.path = `nested/../${nonNormalizedPath.source.cases[0]!.snapshots[0]!.documents[0]!.path}`;
    expect(() => parseMathAuthoringOracleSource(nonNormalizedPath.source)).toThrow("expected a normalized repository-relative document path");

    const disconnected = dependencyFixture();
    disconnected.source.cases[0]!.snapshots[0]!.dependencies = [];
    expect(() => parseMathAuthoringOracleSource(disconnected.source)).toThrow("every document must be reachable from mainFileId");

    const cycle = dependencyFixture();
    const sourceCase = cycle.source.cases[0]!;
    const snapshot = sourceCase.snapshots[0]!;
    snapshot.dependencies.push({ fromFileId: snapshot.documents[1]!.fileId, sourceAnchor: "roles-edge", toFileId: snapshot.mainFileId });
    expect(() => parseMathAuthoringOracleSource(cycle.source)).toThrow("dependency cycle is forbidden");
  });

  test("rejects ambiguous needles, pending review, raw offsets, full contexts, and oversized oracle prose", () => {
    const duplicate = fixtureValue();
    duplicate.source.cases[0]!.namedNeedles[0]!.needle = "x=y";
    duplicate.source.cases[0]!.snapshots[0]!.documents[0]!.content =
      "Select x=y and x=y.";
    finalize(duplicate);
    expect(() => compileFixture(duplicate))
      .toThrow("needle must be unique or declare occurrence");

    const pending = fixtureValue();
    pending.oracle.review.reviewer = "pending-independent-review";
    expect(() => parseMathAuthoringOracle(pending.oracle)).toThrow(
      "expected externally identifiable non-placeholder reviewer/author",
    );

    const rawOffset = fixtureValue();
    (rawOffset.source.cases[0]!.namedNeedles[0] as Record<string, unknown>)
      .startOffset = 2;
    expect(() => parseMathAuthoringOracleSource(rawOffset.source)).toThrow(
      "unexpected keys startOffset",
    );

    const fullContext = fixtureValue();
    (fullContext.oracle.cases[0]!.safety as Record<string, unknown>)
      .StableMathAuthoringContext = {};
    expect(() => parseMathAuthoringOracle(fullContext.oracle)).toThrow(
      "unexpected keys StableMathAuthoringContext",
    );

    const inertReleaseFlag = fixtureValue();
    (inertReleaseFlag.oracle.cases[0]!.advisory.coverageGoals[0] as Record<string, unknown>).releaseRequired = true;
    expect(() => parseMathAuthoringOracle(inertReleaseFlag.oracle)).toThrow(
      "unexpected keys releaseRequired",
    );

    const oversized = fixtureValue();
    oversized.oracle.cases[0]!.advisory.coverageGoals[0]!.rationale = "r".repeat(
      151 * 1024,
    );
    finalize(oversized);
    expect(() => compileFixture(oversized))
      .toThrow("exceeds 150 KiB reviewability guard");

    const selectionMismatch = fixtureValue();
    selectionMismatch.oracle.cases[0]!.safety.formulaAnchor = "tex-0:selection";
    finalize(selectionMismatch);
    expect(() => compileFixture(selectionMismatch)).toThrow(
      "primary formula must strictly contain its source selection",
    );

    const nonWhitespaceMargin = fixtureValue();
    nonWhitespaceMargin.source.cases[0]!.namedNeedles[0]!.needle = "A \nx=y\n";
    finalize(nonWhitespaceMargin);
    expect(() => compileFixture(nonWhitespaceMargin)).toThrow(
      "primary formula margins outside the source selection must be whitespace only",
    );

    const selectionOutsideFormula = fixtureValue();
    const outsideSource = selectionOutsideFormula.source.cases[0]!;
    outsideSource.namedNeedles.push({
      fileId: outsideSource.snapshots[0]!.mainFileId,
      id: "outside-selection",
      needle: "relation",
      snapshotId: "current",
    });
    outsideSource.selections[0]!.anchor = "outside-selection";
    finalize(selectionOutsideFormula);
    expect(() => compileFixture(selectionOutsideFormula)).toThrow(
      "primary formula must strictly contain its source selection",
    );
  });

  test("blocks missing or unexpected authority but reports reviewed non-release omissions", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const missing = observations(fixture);
    missing[0] = {
      ...missing[0]!,
      authoringContext: {
        ...missing[0]!.authoringContext!,
        interpretations: {
          ...missing[0]!.authoringContext!.interpretations,
          hypotheses: [],
        },
      },
    };
    const blocked = evaluateMathAuthoringOracle(compiled, missing);
    expect(blocked.safetyFailures).toContain(
      "tex-0: required authority main expected one exact authority, found 0",
    );
    expect(blocked.safetyFailures).toContain("tex-0: missing hypothesis main");

    const advisoryFixture = fixtureValue();
    for (const item of advisoryFixture.oracle.cases.slice(0, 2)) {
      item.advisory.requiredHypotheses[0]!.releaseRequired = false;
      item.safety.requiredAuthority = [];
    }
    finalize(advisoryFixture);
    const advisoryCompiled = compileFixture(advisoryFixture);
    const advisoryReport = evaluateMathAuthoringOracle(advisoryCompiled, missing);
    expect(advisoryReport.advisoryFindings).toContain(
      "tex-0: missing hypothesis main",
    );
    expect(advisoryReport.suppressedFacets).toContain("interpretations");
  });

  test("does not let a broad selector allowlist duplicate or semantically incomplete authority", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    const current = values[0]!.authoringContext!;
    const reviewed = current.interpretations.hypotheses[0]!;
    values[0] = {
      ...values[0]!,
      authoringContext: {
        ...current,
        interpretations: {
          ...current.interpretations,
          hypotheses: [reviewed, { ...reviewed, hypothesisId: "duplicate-authority", rank: 1 }],
        },
      },
    };
    const report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures).toContain(
      "tex-0: required authority main expected one exact authority, found 2",
    );
    expect(report.safetyFailures.some((failure) => failure.includes("unexpected authority"))).toBe(true);

    const missingIdentity = fixtureValue();
    delete (missingIdentity.oracle.cases[0]!.advisory.requiredHypotheses[0]!.selector as Record<string, unknown>).relationId;
    expect(() => parseMathAuthoringOracle(missingIdentity.oracle)).toThrow(
      "typed-law requires stable relation identity",
    );
  });

  test("requires exact source-authored binding and condition identities without canonicalizing engine proof details", () => {
    const labelsOnly = fixtureValue();
    const labelsOnlyHypothesis = labelsOnly.oracle.cases[0]!.advisory.requiredHypotheses[0]!;
    delete labelsOnlyHypothesis.bindings;
    labelsOnlyHypothesis.bindingRoles = ["value"];
    expect(() => parseMathAuthoringOracle(labelsOnly.oracle)).toThrow("unexpected keys bindingRoles");

    const internalProof = fixtureValue();
    const internalHypothesis = internalProof.oracle.cases[0]!.advisory.requiredHypotheses[0]!;
    (internalHypothesis.bindings as Array<Record<string, unknown>>)[0]!.proof = "typed";
    expect(() => parseMathAuthoringOracle(internalProof.oracle)).toThrow("unexpected keys proof");

    const pairMismatch = fixtureValue();
    const markdownHypothesis = pairMismatch.oracle.cases[1]!.advisory.requiredHypotheses[0]!;
    (markdownHypothesis.bindings as Array<{ parameter: string; symbol: string }>)[0]!.symbol = "z";
    finalize(pairMismatch);
    expect(() => compileFixture(pairMismatch)).toThrow(
      "paired required hypotheses must share source-relative exact contracts",
    );

    const pairSafetyMismatch = fixtureValue();
    pairSafetyMismatch.oracle.cases[1]!.safety.disposition = "partial";
    finalize(pairSafetyMismatch);
    expect(() => compileFixture(pairSafetyMismatch)).toThrow(
      "paired safety envelopes must be source-relative compatible",
    );

    const pairAuthorityMismatch = fixtureValue();
    pairAuthorityMismatch.oracle.cases[1]!.safety.requiredAuthority = [];
    finalize(pairAuthorityMismatch);
    expect(() => compileFixture(pairAuthorityMismatch)).toThrow(
      "paired safety envelopes must be source-relative compatible",
    );

    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const mutations: Array<{
      expected: string;
      mutate: (context: MathAuthoringContext) => void;
    }> = [
      { expected: "bindings mismatch", mutate: (context) => { context.interpretations.hypotheses[0]!.bindings[0]!.parameter = "other"; } },
      { expected: "bindings mismatch", mutate: (context) => { context.interpretations.hypotheses[0]!.bindings[0]!.symbol = "z"; } },
      { expected: "conditions mismatch", mutate: (context) => { context.interpretations.hypotheses[0]!.conditions[0]!.conditionId = "other"; } },
      { expected: "conditions mismatch", mutate: (context) => { context.interpretations.hypotheses[0]!.conditions[0]!.label = "Other condition."; } },
      { expected: "conditions mismatch", mutate: (context) => { context.interpretations.hypotheses[0]!.conditions[0]!.status = "required"; } },
    ];
    for (const mutation of mutations) {
      const values = observations(fixture);
      mutation.mutate(values[0]!.authoringContext!);
      expect(evaluateMathAuthoringOracle(compiled, values).safetyFailures.some((failure) =>
        failure.includes(mutation.expected)
      )).toBe(true);
    }

    const ignoredInternals = observations(fixture);
    for (const mode of ["clean", "incremental"] as const) {
      const context = ignoredInternals.find((item) => item.caseId === "tex-0" && item.mode === mode)!.authoringContext!;
      context.interpretations.hypotheses[0]!.bindings[0]!.proof = "derived";
      context.interpretations.hypotheses[0]!.conditions[0]!.kind = "assumption";
      context.interpretations.hypotheses[0]!.conditions[0]!.subjects = ["different-engine-subject"];
    }
    const ignoredReport = evaluateMathAuthoringOracle(compiled, ignoredInternals);
    expect(ignoredReport.safetyFailures.some((failure) =>
      failure.includes("reviewed support/evidence/bindings/conditions") ||
      failure.includes("bindings mismatch") || failure.includes("conditions mismatch")
    )).toBe(false);
    expect(ignoredReport.pairFailures).toEqual([]);
  });

  test("keeps source meanings protocol-shaped and reviewed conventions relation-qualified", () => {
    const sourceMeaning = fixtureValue();
    const sourceMeaningHypothesis = sourceMeaning.oracle.cases[0]!.advisory.requiredHypotheses[0]!;
    sourceMeaningHypothesis.selector = {
      formulaAnchor: "tex-0:formula",
      kind: "source-meaning",
      label: "reviewed local meaning",
    };
    expect(() => parseMathAuthoringOracle(sourceMeaning.oracle)).toThrow(
      "source-meaning hypotheses must have empty bindings and conditions",
    );

    const reviewedConvention = fixtureValue();
    const reviewedConventionHypothesis = reviewedConvention.oracle.cases[0]!.advisory.requiredHypotheses[0]!;
    reviewedConventionHypothesis.selector = {
      formulaAnchor: "tex-0:formula",
      kind: "reviewed-convention",
      label: "Reviewed convention",
    };
    expect(() => parseMathAuthoringOracle(reviewedConvention.oracle)).toThrow(
      "reviewed-convention requires stable relation identity",
    );
  });

  test("treats supported domain and structural context as advisory, not mathematical authority", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    for (const mode of ["clean", "incremental"] as const) {
      const context = values.find((item) => item.caseId === "tex-0" && item.mode === mode)!.authoringContext!;
      const reviewed = context.interpretations.hypotheses[0]!;
      const { relation: _relation, ...relationless } = reviewed;
      context.interpretations.hypotheses = [
        ...context.interpretations.hypotheses,
        {
          ...relationless,
          bindings: [], conditions: [], hypothesisId: `domain/${mode}`,
          kind: "scoped-domain", label: "Reviewed domain context", rank: 1,
          support: "supported",
        },
        {
          ...relationless,
          bindings: [], conditions: [], hypothesisId: `structure/${mode}`,
          kind: "structural-alternative", label: "Reviewed structural context", rank: 2,
          support: "supported",
        },
        {
          ...reviewed,
          hypothesisId: `convention/${mode}`, kind: "reviewed-convention",
          label: "Tentative reviewed convention", rank: 3, support: "tentative",
        },
      ];
    }
    let report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures.filter((failure) => failure.includes("unexpected authority"))).toEqual([]);

    const supportedLaw = structuredClone(values);
    const context = supportedLaw.find((item) => item.caseId === "tex-0" && item.mode === "clean")!.authoringContext!;
    context.interpretations.hypotheses = [...context.interpretations.hypotheses, {
      ...context.interpretations.hypotheses[0]!,
      hypothesisId: "unexpected/supported-law",
      label: "Unexpected supported law",
      rank: 4,
    }];
    report = evaluateMathAuthoringOracle(compiled, supportedLaw);
    expect(report.safetyFailures).toContain(
      "tex-0: unexpected authority typed-law/Unexpected supported law/supported",
    );

    const supportedConvention = structuredClone(values);
    const conventionContext = supportedConvention.find((item) =>
      item.caseId === "tex-0" && item.mode === "clean"
    )!.authoringContext!;
    conventionContext.interpretations.hypotheses[3]!.support = "supported";
    report = evaluateMathAuthoringOracle(compiled, supportedConvention);
    expect(report.safetyFailures).toContain(
      "tex-0: unexpected authority reviewed-convention/Tentative reviewed convention/supported",
    );

    const explicitDomain = structuredClone(values);
    const explicitContext = explicitDomain.find((item) => item.caseId === "tex-0" && item.mode === "clean")!.authoringContext!;
    explicitContext.interpretations.hypotheses[1]!.support = "explicit";
    report = evaluateMathAuthoringOracle(compiled, explicitDomain);
    expect(report.safetyFailures.some((failure) =>
      failure.includes("hypotheses[1].support: unsafe authority escalation")
    )).toBe(true);
  });

  test("keeps reviewed contradictions distinct from positive authority", () => {
    const fixture = fixtureValue();
    for (const item of fixture.oracle.cases.slice(0, 2)) {
      item.advisory.requiredHypotheses[0]!.supportAllowed = ["contradicted"];
      item.safety.requiredAuthority = [];
      item.safety.requiredContradictions = ["main"];
    }
    const first = fixture.oracle.cases[0]!;
    finalize(fixture);
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    const current = values[0]!.authoringContext!;
    const hypothesis = current.interpretations.hypotheses[0]!;
    values[0] = {
      ...values[0]!,
      authoringContext: {
        ...current,
        interpretations: {
          ...current.interpretations,
          hypotheses: [{ ...hypothesis, support: "contradicted" }],
        },
      },
    };
    let report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures.some((failure) => failure.startsWith("tex-0:") && failure.includes("required authority main"))).toBe(false);
    expect(report.safetyFailures.some((failure) => failure.startsWith("tex-0:") && failure.includes("required contradiction main"))).toBe(false);

    values[0] = { ...values[0]!, authoringContext: current };
    report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures).toContain(
      "tex-0: required contradiction main expected one exact contradiction, found 0",
    );
    expect(report.safetyFailures.some((failure) => failure.includes("unexpected authority"))).toBe(true);
  });

  test("matches exact evidence-anchor multisets independent of source order", () => {
    const fixture = fixtureValue();
    for (const source of fixture.source.cases.slice(0, 2)) {
      source.namedNeedles.push({
        fileId: source.snapshots[0]!.mainFileId,
        id: "secondary",
        needle: "relation",
        snapshotId: "current",
      });
      (fixture.oracle.evidence[`evidence-${source.id}`] as { anchors: string[] }).anchors.push(`${source.id}:secondary`);
    }
    finalize(fixture);
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    for (const observation of values.filter((item) => item.caseId === "tex-0" || item.caseId === "md-0")) {
      const source = fixture.source.cases.find((item) => item.id === observation.caseId)!;
      const secondary = selectionFor(source, "current", "relation");
      const evidence = observation.authoringContext!.interpretations.hypotheses[0]!.evidence[0]!;
      const secondaryAnchor = {
        ...secondary,
        generation: "authored",
        lifecycle: "current",
        scopePath: [],
      } as const;
      evidence.sourceAnchors = observation.caseId === "tex-0"
        ? [secondaryAnchor, ...evidence.sourceAnchors]
        : [...evidence.sourceAnchors, secondaryAnchor];
    }
    const report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures).toEqual([]);
    expect(report.pairFailures).toEqual([]);

    const reversedEvidence = structuredClone(values);
    for (const caseId of ["tex-0", "md-0"] as const) {
      const context = reversedEvidence.find((item) => item.caseId === caseId && item.mode === "clean")!.authoringContext!;
      const reviewed = context.interpretations.hypotheses[0]!.evidence[0]!;
      const extra = {
        ...reviewed,
        evidence: { ...reviewed.evidence, ruleId: "test/secondary" },
        sourceAnchors: [...reviewed.sourceAnchors].reverse(),
      };
      context.interpretations.hypotheses[0]!.evidence = caseId === "tex-0"
        ? [reviewed, extra]
        : [extra, reviewed];
      const references = [reviewed, extra].map(({ evidence, sourceAnchors }) => ({ evidence, sourceAnchors }));
      context.interpretations.analysisLimits = [{
        evidence: caseId === "tex-0" ? references : [...references].reverse(),
        kind: "engine-limit",
      }];
      context.lifecycle.engineLimited = true;
    }
    const reversedReport = evaluateMathAuthoringOracle(compiled, reversedEvidence);
    expect(reversedReport.safetyFailures.filter((failure) => failure.includes("malformed authoring context"))).toEqual([]);
    expect(reversedReport.pairFailures).toEqual([]);

    const missingAnchor = structuredClone(values);
    missingAnchor[0]!.authoringContext!.interpretations.hypotheses[0]!.evidence[0]!.sourceAnchors = [];
    expect(evaluateMathAuthoringOracle(compiled, missingAnchor).safetyFailures).not.toEqual([]);
  });

  test("supports exact mixed-lifecycle evidence while preserving the legacy uniform form", () => {
    const legacy = finalizedFixture();
    expect(() => compileFixture(legacy)).not.toThrow();

    const fixture = fixtureValue();
    for (const source of fixture.source.cases.slice(0, 2)) {
      source.namedNeedles.push({
        fileId: source.snapshots[0]!.mainFileId,
        id: "secondary",
        needle: "relation",
        snapshotId: "current",
      });
      const reviewed = fixture.oracle.evidence[`evidence-${source.id}`] as Record<string, unknown>;
      reviewed.role = "contradicting";
      delete reviewed.anchors;
      delete reviewed.generation;
      delete reviewed.lifecycle;
      reviewed.anchorStates = [
        { anchor: `${source.id}:formula`, generation: "authored", lifecycle: "current" },
        { anchor: `${source.id}:secondary`, generation: "authored", lifecycle: "retracted" },
      ];
      const constraint = fixture.oracle.cases.find((item) => item.id === source.id);
      if (!constraint) throw new Error("mixed evidence constraint missing");
      const required = constraint.advisory.requiredHypotheses[0] as Record<string, unknown>;
      required.supportAllowed = ["contradicted"];
      constraint.safety.requiredAuthority = [];
      (constraint.safety as Record<string, unknown>).requiredContradictions = ["main"];
    }
    finalize(fixture);
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    for (const observation of values.filter((item) => item.caseId === "tex-0" || item.caseId === "md-0")) {
      const source = fixture.source.cases.find((item) => item.id === observation.caseId);
      if (!source || !observation.authoringContext) throw new Error("mixed evidence test case missing");
      const secondary = selectionFor(source, "current", "relation");
      const hypothesis = observation.authoringContext.interpretations.hypotheses[0]!;
      hypothesis.support = "contradicted";
      hypothesis.evidence[0]!.role = "contradicting";
      hypothesis.evidence[0]!.sourceAnchors = [
        ...hypothesis.evidence[0]!.sourceAnchors,
        {
          ...secondary,
          generation: "authored",
          lifecycle: "retracted",
          scopePath: [],
        },
      ];
    }
    const report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures).toEqual([]);
    expect(report.pairFailures).toEqual([]);

    const wrongLifecycle = structuredClone(values);
    wrongLifecycle.find((item) => item.caseId === "tex-0" && item.mode === "clean")!
      .authoringContext!.interpretations.hypotheses[0]!.evidence[0]!
      .sourceAnchors[1]!.lifecycle = "current";
    expect(evaluateMathAuthoringOracle(compiled, wrongLifecycle).safetyFailures).not.toEqual([]);

    const changedConstraint = structuredClone(fixture);
    const changedStates = (changedConstraint.oracle.evidence["evidence-tex-0"] as {
      anchorStates: Array<{ anchor: string; generation: string; lifecycle: string }>;
    }).anchorStates;
    changedStates[1]!.lifecycle = "current";
    expect(mathAuthoringOracleConstraintDigest(parseMathAuthoringOracle(changedConstraint.oracle)))
      .not.toBe(mathAuthoringOracleConstraintDigest(compiled.oracle));
  });

  test("rejects mixed evidence XOR, duplicate or cross-case states, and pair drift", () => {
    const bothForms = fixtureValue();
    const both = bothForms.oracle.evidence["evidence-tex-0"] as Record<string, unknown>;
    both.anchorStates = [{ anchor: "tex-0:formula", generation: "authored", lifecycle: "current" }];
    expect(() => parseMathAuthoringOracle(bothForms.oracle)).toThrow("mutually exclusive");

    const incompleteLegacy = fixtureValue();
    delete (incompleteLegacy.oracle.evidence["evidence-tex-0"] as Record<string, unknown>).lifecycle;
    expect(() => parseMathAuthoringOracle(incompleteLegacy.oracle)).toThrow("complete anchors/generation/lifecycle");

    const emptyMixed = fixtureValue();
    const empty = emptyMixed.oracle.evidence["evidence-tex-0"] as Record<string, unknown>;
    delete empty.anchors;
    delete empty.generation;
    delete empty.lifecycle;
    empty.anchorStates = [];
    expect(() => parseMathAuthoringOracle(emptyMixed.oracle)).toThrow("expected at least one anchor state");

    const duplicateMixed = fixtureValue();
    const duplicate = duplicateMixed.oracle.evidence["evidence-tex-0"] as Record<string, unknown>;
    delete duplicate.anchors;
    delete duplicate.generation;
    delete duplicate.lifecycle;
    duplicate.anchorStates = [
      { anchor: "tex-0:formula", generation: "authored", lifecycle: "current" },
      { anchor: "tex-0:formula", generation: "authored", lifecycle: "retracted" },
    ];
    expect(() => parseMathAuthoringOracle(duplicateMixed.oracle)).toThrow("duplicate tex-0:formula");

    const crossCase = fixtureValue();
    const cross = crossCase.oracle.evidence["evidence-tex-0"] as Record<string, unknown>;
    delete cross.anchors;
    delete cross.generation;
    delete cross.lifecycle;
    cross.anchorStates = [{ anchor: "md-0:formula", generation: "authored", lifecycle: "current" }];
    finalize(crossCase);
    expect(() => compileFixture(crossCase)).toThrow("unknown or cross-case anchor");

    const pairDrift = fixtureValue();
    for (const caseId of ["tex-0", "md-0"]) {
      const evidence = pairDrift.oracle.evidence[`evidence-${caseId}`] as Record<string, unknown>;
      delete evidence.anchors;
      delete evidence.generation;
      delete evidence.lifecycle;
      evidence.anchorStates = [{
        anchor: `${caseId}:formula`,
        generation: "authored",
        lifecycle: caseId === "tex-0" ? "current" : "retracted",
      }];
    }
    finalize(pairDrift);
    expect(() => compileFixture(pairDrift)).toThrow("paired required hypotheses must share source-relative exact contracts");
  });

  test("derives and enforces protocol-owned cap metadata at the 16/+1 boundary", () => {
    const boundary = capFixture(15);
    expect(() => compileFixture(boundary)).toThrow(
      "cap requires more than 16 distinct reviewed semantic keys",
    );

    const fixture = capFixture(16);
    const compiled = compileFixture(fixture);
    const values = cappedObservations(fixture, compiled);
    let report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures.filter((failure) => failure.includes("cap"))).toEqual([]);
    expect(report.pairFailures).toEqual([]);

    const cleanTex = values.findIndex((item) => item.caseId === "tex-0" && item.mode === "clean");
    const withoutMetadata = structuredClone(values);
    delete withoutMetadata[cleanTex]!.authoringContext!.interpretations.candidateCap;
    report = evaluateMathAuthoringOracle(compiled, withoutMetadata);
    expect(report.safetyFailures).toContain("tex-0: missing protocol-owned candidateCap metadata");

    const wrongDigest = structuredClone(values);
    wrongDigest[cleanTex]!.authoringContext!.interpretations.candidateCap!.preCapSemanticKeyDigest = "b".repeat(64);
    report = evaluateMathAuthoringOracle(compiled, wrongDigest);
    expect(report.safetyFailures).toContain("tex-0: candidateCap count/digest mismatch");

    const boundaryMetadata = structuredClone(values);
    boundaryMetadata[cleanTex]!.authoringContext!.interpretations.candidateCap!.candidateCountBeforeCap = 16;
    report = evaluateMathAuthoringOracle(compiled, boundaryMetadata);
    expect(report.safetyFailures.some((failure) => failure.includes("candidateCountBeforeCap must be a u32 integer above the hypothesis limit"))).toBe(true);

    const unreviewed = structuredClone(values);
    unreviewed[cleanTex]!.authoringContext!.interpretations.hypotheses[1]!.label = "Unreviewed replacement";
    report = evaluateMathAuthoringOracle(compiled, unreviewed);
    expect(report.safetyFailures).toContain("tex-0: exposed hypothesis absent from reviewed pre-cap identities");

    const aboveU32 = structuredClone(values);
    aboveU32[cleanTex]!.authoringContext!.interpretations.candidateCap!.candidateCountBeforeCap = 0x1_0000_0000;
    report = evaluateMathAuthoringOracle(compiled, aboveU32);
    expect(report.safetyFailures.some((failure) =>
      failure.includes("candidateCountBeforeCap must be a u32 integer above the hypothesis limit")
    )).toBe(true);

    const unexpectedMetadata = observations(finalizedFixture());
    unexpectedMetadata[0]!.authoringContext!.interpretations.candidateCap = {
      candidateCountBeforeCap: 17,
      preCapSemanticKeyDigest: "b".repeat(64),
    };
    report = evaluateMathAuthoringOracle(compileFixture(finalizedFixture()), unexpectedMetadata);
    expect(report.safetyFailures).toContain(
      "tex-0:current:clean: candidateCap envelope does not match the reviewed case contract",
    );

    const generalTruncation = fixtureValue();
    for (const item of generalTruncation.oracle.cases.slice(0, 2)) {
      item.safety.truncated = true;
      item.safety.interpretationsTruncated = true;
      item.safety.lifecycle.capped = true;
      item.safety.limits = [{ evidence: [`evidence-${item.id}`], kind: "engine-limit" }];
    }
    finalize(generalTruncation);
    expect(() => compileFixture(generalTruncation)).not.toThrow();

    const candidateLimitWithoutCap = fixtureValue();
    candidateLimitWithoutCap.oracle.cases[0]!.safety.limits = [{
      evidence: ["evidence-tex-0"], kind: "candidate-set-capped",
    }];
    finalize(candidateLimitWithoutCap);
    expect(() => compileFixture(candidateLimitWithoutCap)).toThrow(
      "cap must exist iff one candidate-set-capped limit and both truncation flags plus lifecycle.capped are present",
    );

    for (const removePart of ["limit", "authoring", "interpretations", "lifecycle"] as const) {
      const incomplete = capFixture(16);
      const safety = incomplete.oracle.cases[0]!.safety;
      if (removePart === "limit") safety.limits = [];
      if (removePart === "authoring") safety.truncated = false;
      if (removePart === "interpretations") safety.interpretationsTruncated = false;
      if (removePart === "lifecycle") safety.lifecycle.capped = false;
      finalize(incomplete);
      expect(() => compileFixture(incomplete)).toThrow(
        "cap must exist iff one candidate-set-capped limit and both truncation flags plus lifecycle.capped are present",
      );
    }
  });

  test("requires clean/incremental parity for every planned present snapshot independent of facets", () => {
    const fixture = finalizedFixture();
    for (const item of fixture.oracle.cases) item.facets = item.facets.filter((facet) => facet !== "clean-incremental");
    finalize(fixture);
    const values = observations(fixture);
    const incrementalIndex = values.findIndex((item) => item.caseId === "tex-0" && item.mode === "incremental");
    const incremental = values[incrementalIndex]!;
    values[incrementalIndex] = { ...incremental, authoringContext: { ...incremental.authoringContext!, disposition: "partial" } };
    expect(evaluateMathAuthoringOracle(compileFixture(fixture), values).transitionFailures).toContain(
      "tex-0: current clean/incremental mismatch",
    );
  });

  test("checks reviewed before-after authority and removed-anchor deltas", () => {
    const fixture = fixtureValue();
    const source = fixture.source.cases[0]!;
    source.selections[0]!.snapshotId = "before";
    source.snapshots = [
      { ...source.snapshots[0]!, id: "before" },
      { dependencies: [], documents: [{ ...source.snapshots[0]!.documents[0]!, content: "A \nx=y\n relation without declaration." }], id: "after", mainFileId: source.snapshots[0]!.mainFileId },
      { dependencies: [], documents: [{ ...source.snapshots[0]!.documents[0]!, content: "Formula removed here." }], id: "removed", mainFileId: source.snapshots[0]!.mainFileId },
    ];
    for (const needle of source.namedNeedles) needle.snapshotId = "before";
    source.namedNeedles.push(
      { ...source.namedNeedles[0]!, id: "formula-after", snapshotId: "after" },
      { ...source.namedNeedles[1]!, id: "selection-after", parentAnchor: "formula-after", snapshotId: "after" },
      { ...source.namedNeedles[0]!, id: "removed-cursor", needle: "removed", snapshotId: "removed" },
    );
    source.selections.push(
      { anchor: "selection-after", id: "after-selection", snapshotId: "after" },
      { anchor: "removed-cursor", id: "removed-selection", snapshotId: "removed" },
    );
    const markdownSource = fixture.source.cases[1]!;
    const markdownFileId = markdownSource.snapshots[0]!.mainFileId;
    markdownSource.snapshots = source.snapshots.map((snapshot) => ({
      dependencies: [],
      documents: snapshot.documents.map((document) => ({ ...document, fileId: markdownFileId, path: markdownFileId })),
      id: snapshot.id,
      mainFileId: markdownFileId,
    }));
    markdownSource.namedNeedles = source.namedNeedles.map((needle) => ({ ...needle, fileId: markdownFileId }));
    markdownSource.selections = source.selections.map((selection) => ({ ...selection }));
    const first = fixture.oracle.cases[0]!;
    const beforeLifecycle = { ...first.safety.lifecycle };
    const afterLifecycle = { ...beforeLifecycle, documentVersion: 2 };
    first.transition = {
      before: {
        disposition: "established",
        formulaAnchor: "tex-0:formula",
        lifecycle: beforeLifecycle,
        requiredAnchors: ["tex-0:formula"],
        requiredAuthority: ["main"],
        snapshotId: "before",
      },
      after: {
        disposition: "established",
        forbiddenAnchors: ["tex-0:formula"],
        forbiddenAuthority: [{ formulaAnchor: "tex-0:formula", kind: "typed-law", label: "Reviewed relation", relationId: "test/relation" }],
        formulaAnchor: "tex-0:formula-after",
        lifecycle: afterLifecycle,
        requiredMissingDiscriminators: [],
        snapshotId: "after",
      },
      cleanIncremental: true,
      removed: { context: "absent", selectionAnchor: "tex-0:removed-cursor", snapshotId: "removed" },
    };
    fixture.oracle.cases[1]!.transition = JSON.parse(
      JSON.stringify(first.transition).replaceAll("tex-0", "md-0"),
    );
    finalize(fixture);
    const compiled = compileFixture(fixture);

    const transitionPairMismatch = structuredClone(fixture);
    const mismatchedTransition = transitionPairMismatch.oracle.cases[1]!.transition as {
      before: { disposition: string };
    };
    mismatchedTransition.before.disposition = "partial";
    finalize(transitionPairMismatch);
    expect(() => compileFixture(transitionPairMismatch)).toThrow(
      "paired safety envelopes must be source-relative compatible",
    );
    const base = contextFor(source, "before");
    const values = observations(fixture).filter((item) => item.caseId !== "tex-0");
    values.push(
      { authoringContext: base, caseId: "tex-0", mode: "clean", selection: selectionFor(source, "before", "x=y"), snapshotId: "before" },
      { authoringContext: base, caseId: "tex-0", mode: "incremental", selection: selectionFor(source, "before", "x=y"), snapshotId: "before" },
      { authoringContext: base, caseId: "tex-0", mode: "clean", selection: selectionFor(source, "after", "x=y"), snapshotId: "after" },
      { authoringContext: base, caseId: "tex-0", mode: "incremental", selection: selectionFor(source, "after", "x=y"), snapshotId: "after" },
      { caseId: "tex-0", mode: "clean", selection: selectionFor(source, "removed", "removed"), snapshotId: "removed" },
      { caseId: "tex-0", mode: "incremental", selection: selectionFor(source, "removed", "removed"), snapshotId: "removed" },
    );
    const transitionPairDivergence = structuredClone(values);
    const markdownBeforeIndex = transitionPairDivergence.findIndex((item) =>
      item.caseId === "md-0" && item.snapshotId === "before" && item.mode === "clean"
    );
    const markdownBefore = transitionPairDivergence[markdownBeforeIndex]!;
    transitionPairDivergence[markdownBeforeIndex] = {
      ...markdownBefore,
      authoringContext: {
        ...markdownBefore.authoringContext!,
        disposition: "partial",
      },
    };
    expect(evaluateMathAuthoringOracle(compiled, transitionPairDivergence).pairFailures).toContain(
      "pair-0: TeX/Markdown semantic parity mismatch",
    );
    const report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.transitionFailures).toContain(
      'tex-0: after retained removed authority {"formulaAnchor":"tex-0:formula","kind":"typed-law","label":"Reviewed relation","relationId":"test/relation"}',
    );
    expect(report.transitionFailures).toContain(
      "tex-0: after retained unexpected authority typed-law/Reviewed relation/supported",
    );
    expect(report.transitionFailures).toContain(
      "tex-0: after retained removed anchor tex-0:formula",
    );
    expect(report.transitionFailures).toContain("tex-0: after formula anchor mismatch");
    expect(report.transitionFailures).toContain("tex-0: after lifecycle mismatch");

    const missingRemovedIncremental = values.filter((item) =>
      !(item.caseId === "tex-0" && item.snapshotId === "removed" && item.mode === "incremental")
    );
    expect(evaluateMathAuthoringOracle(compiled, missingRemovedIncremental).transitionFailures).toContain(
      "tex-0: removed snapshot missing incremental absence observation",
    );

    const nonChronological = structuredClone(fixture);
    for (const sourceCase of nonChronological.source.cases.slice(0, 2)) {
      [sourceCase.snapshots[1], sourceCase.snapshots[2]] = [sourceCase.snapshots[2]!, sourceCase.snapshots[1]!];
    }
    finalize(nonChronological);
    expect(() => compileFixture(nonChronological)).toThrow(
      "transition snapshots must be chronological before < after < removed",
    );

    const missingTransitionSelection = structuredClone(fixture);
    missingTransitionSelection.source.cases[0]!.selections = missingTransitionSelection.source.cases[0]!.selections.filter((selection) => selection.id !== "after-selection");
    finalize(missingTransitionSelection);
    expect(() => compileFixture(missingTransitionSelection)).toThrow(
      "transition snapshot after requires exactly one explicit source selection",
    );

    const ambiguousTransitionSelection = structuredClone(fixture);
    ambiguousTransitionSelection.source.cases[0]!.selections.push({
      anchor: "selection-after",
      id: "duplicate-after-selection",
      snapshotId: "after",
    });
    finalize(ambiguousTransitionSelection);
    expect(() => compileFixture(ambiguousTransitionSelection)).toThrow(
      "transition snapshot after requires exactly one explicit source selection",
    );
  });

  test("detects semantic rather than nominal TeX-Markdown pair mismatches", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    const markdownIndex = values.findIndex(
      (item) => item.caseId === "md-0" && item.mode === "clean",
    );
    const markdown = values[markdownIndex]!;
    const hypothesis = markdown.authoringContext!.interpretations.hypotheses[0]!;
    values[markdownIndex] = {
      ...markdown,
      authoringContext: {
        ...markdown.authoringContext!,
        interpretations: {
          ...markdown.authoringContext!.interpretations,
          hypotheses: [{ ...hypothesis, support: "tentative" }],
        },
      },
    };
    expect(evaluateMathAuthoringOracle(compiled, values).pairFailures).toEqual([
      "pair-0: TeX/Markdown semantic parity mismatch",
    ]);

    const labelValues = observations(fixture);
    const labelMarkdown = labelValues[markdownIndex]!;
    const labelHypothesis = labelMarkdown.authoringContext!.interpretations.hypotheses[0]!;
    labelValues[markdownIndex] = {
      ...labelMarkdown,
      authoringContext: {
        ...labelMarkdown.authoringContext!,
        interpretations: {
          ...labelMarkdown.authoringContext!.interpretations,
          hypotheses: [{ ...labelHypothesis, label: "Different structural meaning" }],
        },
      },
    };
    expect(evaluateMathAuthoringOracle(compiled, labelValues).pairFailures).toEqual([
      "pair-0: TeX/Markdown semantic parity mismatch",
    ]);
  });

  test("rejects duplicate and unexpected observations instead of last-write-wins", () => {
    const fixture = finalizedFixture();
    const compiled = compileFixture(fixture);
    const values = observations(fixture);
    values.push(structuredClone(values[0]!));
    values.push({ caseId: "tex-0", mode: "clean", selection: values[0]!.selection, snapshotId: "invented" });
    const report = evaluateMathAuthoringOracle(compiled, values);
    expect(report.safetyFailures).toContain("observations: duplicate tex-0:current:clean");
    expect(report.safetyFailures).toContain("observations: unexpected tex-0:invented:clean");

    const wrongSelection = observations(fixture);
    wrongSelection[0] = {
      ...wrongSelection[0]!,
      selection: { ...wrongSelection[0]!.selection, documentVersion: 99 },
    };
    expect(evaluateMathAuthoringOracle(compiled, wrongSelection).safetyFailures).toContain(
      "tex-0:current:clean: selection receipt mismatch",
    );

    const plan = mathAuthoringExpectedObservationPlan(compiled);
    expect(plan).toHaveLength(40);
    expect(plan[0]).toMatchObject({
      caseId: "tex-0", context: "present", mode: "clean",
      selectionAnchorId: "tex-0:selection", snapshotId: "current",
    });
  });

  test("normalizes removed context only when every stale authoring surface is absent", () => {
    const fixture = finalizedFixture();
    const current = observations(fixture)[0]!.authoringContext!;
    const { formula: _formula, ...withoutFormula } = current;
    const safelyAbsent: MathAuthoringContext = {
      ...withoutFormula,
      claimEvidence: [], conditions: [], disposition: "unsupported", equationLinks: [],
      interpretations: {
        analysisLimits: [], exhaustiveness: "bounded-open-world", hypotheses: [],
        missingDiscriminators: [], truncated: false,
      },
      lifecycle: { ...current.lifecycle, capped: false, engineLimited: false, retracted: false },
      notationOccurrences: [], requirements: [], truncated: false,
    };
    expect(isMathAuthoringRemovedContextSafelyAbsent(undefined)).toBe(true);
    expect(isMathAuthoringRemovedContextSafelyAbsent(safelyAbsent)).toBe(true);
    expect(isMathAuthoringRemovedContextSafelyAbsent({ ...safelyAbsent, formula: current.formula })).toBe(false);
    expect(isMathAuthoringRemovedContextSafelyAbsent({
      ...safelyAbsent,
      interpretations: { ...safelyAbsent.interpretations, hypotheses: current.interpretations.hypotheses },
    })).toBe(false);
    expect(isMathAuthoringRemovedContextSafelyAbsent({ ...safelyAbsent, staleAuthority: true })).toBe(false);
  });

  test("binds canonical source identity and an external independent review attestation", () => {
    const invalidDate = fixtureValue();
    invalidDate.oracle.review.reviewedAt = "2026-02-30";
    expect(() => parseMathAuthoringOracle(invalidDate.oracle)).toThrow("valid YYYY-MM-DD calendar date");

    const selfReviewed = fixtureValue();
    selfReviewed.oracle.review.reviewer = selfReviewed.oracle.review.author;
    expect(() => parseMathAuthoringOracle(selfReviewed.oracle)).toThrow("reviewer must be independent from author");

    const caseChangedSelfReview = fixtureValue();
    caseChangedSelfReview.oracle.review.reviewer = caseChangedSelfReview.oracle.review.author.toUpperCase();
    expect(() => parseMathAuthoringOracle(caseChangedSelfReview.oracle)).toThrow("reviewer must be independent from author");

    const staleSource = finalizedFixture();
    staleSource.oracle.sourceSha256 = "b".repeat(64);
    expect(() => compileFixture(staleSource)).toThrow("canonical path, fixture identity, or digest mismatch");

    const fixture = finalizedFixture();
    const oracle = parseMathAuthoringOracle(fixture.oracle);
    const attestation = reviewAttestation(oracle);
    fixture.oracle.cases[0]!.safety.disposition = "unsupported";
    expect(() => compileMathAuthoringOracle(fixture.source, fixture.oracle, attestation)).toThrow(
      "oracle constraint binding mismatch",
    );

    const denied = { ...attestation, verdict: "denied" };
    expect(() => compileMathAuthoringOracle(finalizedFixture().source, finalizedFixture().oracle, denied)).toThrow(
      "expected approved",
    );
  });

  test("requires generated subnodes to have exact generated evidence anchors while the formula stays authored", () => {
    const fixture = fixtureValue();
    const source = fixture.source.cases[0]!;
    source.namedNeedles.push(
      { fileId: source.snapshots[0]!.mainFileId, id: "formula-child", needle: "x", parentAnchor: "formula", snapshotId: "current" },
      { fileId: source.snapshots[0]!.mainFileId, id: "outside", needle: "relation", snapshotId: "current" },
    );
    fixture.oracle.evidence["generated-evidence-tex-0"] = {
      anchors: ["tex-0:formula-child"], generation: "generated", kind: "source-structure",
      lifecycle: "current", provenance: "typed-structure", role: "supporting",
      ruleId: "test/generated", strength: "hard",
    };
    fixture.oracle.cases[0]!.safety.generatedSubnodes = [{
      anchor: "tex-0:formula-child", evidence: ["generated-evidence-tex-0"],
    }];
    finalize(fixture);
    const compiled = compileFixture(fixture);
    const report = evaluateMathAuthoringOracle(compiled, observations(fixture));
    expect(fixture.oracle.cases[0]!.safety.lifecycle.generation).toBe("authored");
    expect(report.safetyFailures).toContain(
      "tex-0: generated subnode tex-0:formula-child missing exact generated provenance generated-evidence-tex-0",
    );

    const sameAsFormula = structuredClone(fixture);
    sameAsFormula.oracle.cases[0]!.safety.generatedSubnodes = [{
      anchor: "tex-0:formula", evidence: ["generated-evidence-tex-0"],
    }];
    finalize(sameAsFormula);
    expect(() => compileFixture(sameAsFormula)).toThrow("must be a strict nested descendant of the safety formula");

    const outsideFormula = structuredClone(fixture);
    outsideFormula.oracle.cases[0]!.safety.generatedSubnodes = [{
      anchor: "tex-0:outside", evidence: ["generated-evidence-tex-0"],
    }];
    (outsideFormula.oracle.evidence["generated-evidence-tex-0"] as { anchors: string[] }).anchors = ["tex-0:outside"];
    finalize(outsideFormula);
    expect(() => compileFixture(outsideFormula)).toThrow("must be a strict nested descendant of the safety formula");
  });

  test("diagnostic projection is content-addressed and never part of canonical review input", () => {
    const fixture = finalizedFixture();
    const values = observations(fixture);
    const artifact = mathAuthoringDiagnosticArtifact(values);
    expect(artifact.artifactId).toBe(`sha256:${artifact.sha256}`);
    expect(mathAuthoringDiagnosticArtifactPath(artifact)).toBe(
      `.artifacts/math-authoring-oracle/${artifact.sha256}.json`,
    );
    expect(() => mathAuthoringDiagnosticArtifactPath({ artifactId: "sha256:bad", sha256: "bad" })).toThrow(
      "invalid content address",
    );
    expect(artifact.content).toContain('"stable"');
    expect(artifact.content).toContain('"selection"');
    expect(mathAuthoringDiagnosticArtifact([...values].reverse()).sha256).toBe(artifact.sha256);
    const changedReceipt = structuredClone(values);
    changedReceipt[0] = {
      ...changedReceipt[0]!,
      selection: { ...changedReceipt[0]!.selection, documentVersion: 99 },
    };
    expect(mathAuthoringDiagnosticArtifact(changedReceipt).sha256).not.toBe(artifact.sha256);
    expect(JSON.stringify(fixture.oracle)).not.toContain("authoringContext");
  });
});

interface FixtureValue {
  oracle: {
    cases: Array<Record<string, unknown> & {
      advisory: {
        allowedExtras: { anchorAllowlist: string[]; kinds: string[]; maxCount: number; provenances: string[]; supportAllowed: string[] };
        coverageGoals: Array<{ facet: string; rationale: string }>;
        knownMisses: unknown[]; relativeOrder: unknown[];
        requiredHypotheses: Array<Record<string, unknown> & { releaseRequired: boolean }>;
        requiredMissingDiscriminators: unknown[]; requiredRequirements: unknown[];
      };
      cap?: Record<string, unknown>;
      facets: string[];
      selectionId: string;
      safety: Record<string, unknown> & {
        interpretationsTruncated: boolean;
        lifecycle: { capped: boolean; documentVersion: number; editable: boolean; engineLimited: boolean; freshness: string; generation: string; retracted: boolean };
        limits: unknown[];
        requiredAuthority: string[];
        truncated: boolean;
      };
      transition?: Record<string, unknown>;
    }>;
    evidence: Record<string, unknown>;
    pairs: unknown[];
    review: { attestationDigest: string; author: string; digest: string; reviewFixture: string; reviewedAt: string; reviewer: string };
    schemaVersion: number;
    sourceFixture: string;
    sourceFixtureId: string;
    sourceSha256: string;
  };
  source: {
    cases: Array<{
      id: string; language: string; pairId: string;
      namedNeedles: Array<{
        fileId: string; id: string; needle: string; snapshotId: string;
        parentAnchor?: string;
        within?: { anchor: string; needle: string; occurrence?: number };
      }>;
      selections: Array<{ anchor: string; id: string; snapshotId: string }>;
      snapshots: Array<{
        dependencies: Array<{ fromFileId: string; sourceAnchor: string; toFileId: string }>;
        documents: Array<{ content: string; fileId: string; path: string }>;
        id: string;
        mainFileId: string;
      }>;
    }>;
    fixtureId: string;
    pairs: Array<{ id: string; latexCaseId: string; markdownCaseId: string }>;
    schemaVersion: number;
  };
}

function fixtureValue(): FixtureValue {
  const sourceCases = Array.from({ length: 10 }, (_, pair) => [
    sourceCase(`tex-${pair}`, "latex", `pair-${pair}.tex`),
    sourceCase(`md-${pair}`, "markdown", `pair-${pair}.md`),
  ]).flat();
  const evidence = Object.fromEntries(sourceCases.map((item) => [
    `evidence-${item.id}`,
    {
      anchors: [`${item.id}:formula`],
      generation: "authored",
      kind: "source-structure",
      lifecycle: "current",
      provenance: "typed-structure",
      role: "supporting",
      ruleId: "test/relation",
      strength: "hard",
    },
  ]));
  const cases = sourceCases.map((item) => ({
    advisory: {
      allowedExtras: {
        anchorAllowlist: [`${item.id}:formula`],
        kinds: [], maxCount: 0, provenances: [], supportAllowed: [],
      },
      coverageGoals: [{ facet: "interpretations", rationale: "The reviewed relation is the public value of this tiny fixture." }],
      knownMisses: [], relativeOrder: [],
      requiredHypotheses: [{
        bindings: [{ parameter: "value", symbol: "x" }],
        conditions: [{ conditionId: "same-context", label: "Same context.", status: "verified" }],
        dependentFacets: ["interpretations"], evidence: [`evidence-${item.id}`],
        id: "main", releaseRequired: true,
        selector: { formulaAnchor: `${item.id}:formula`, kind: "typed-law", label: "Reviewed relation", relationId: "test/relation" },
        supportAllowed: ["supported"],
      }],
      requiredMissingDiscriminators: [], requiredRequirements: [],
    },
    facets: ["clean-incremental", "interpretations", "lifecycle"],
    id: item.id,
    safety: {
      claims: [], disposition: "established", equationLinks: [],
      forbiddenAuthority: [], formulaAnchor: `${item.id}:formula`,
      interpretationsTruncated: false,
      lifecycle: {
        capped: false, documentVersion: 1, editable: true, engineLimited: false,
        freshness: "current", generation: "authored", retracted: false,
      },
      generatedSubnodes: [], limits: [], noUnexpectedAuthority: true,
      noUnexpectedContradictions: true, notation: [],
      requiredAuthority: ["main"], requiredContradictions: [],
      truncated: false,
    },
    selectionId: "primary",
    sourceCaseId: item.id,
  }));
  return {
    oracle: {
      cases, evidence,
      pairs: Array.from({ length: 10 }, (_, pair) => ({
        compare: {
          authority: "exact",
          hypotheses: ["kind", "label", "relationId", "formulaAnchor", "support", "bindings", "conditions", "evidence"],
          lifecycle: "exact", limits: "exact", ordering: "required-relative",
        },
        id: `pair-${pair}`, markdownCaseId: `md-${pair}`, texCaseId: `tex-${pair}`,
      })),
      review: {
        attestationDigest: "a".repeat(64),
        author: "agent:/root/oracle_author",
        digest: "0".repeat(64),
        reviewFixture: "fixtures/challenge/math-authoring-oracle-review-v2.json",
        reviewedAt: "2026-08-20",
        reviewer: "agent:/root/performance_review",
      },
      schemaVersion: 2,
      sourceFixture: "fixtures/challenge/math-authoring-oracle-source-v2.json",
      sourceFixtureId: "semath-math-authoring-public-source-v2",
      sourceSha256: "0".repeat(64),
    },
    source: {
      cases: sourceCases,
      fixtureId: "semath-math-authoring-public-source-v2",
      pairs: Array.from({ length: 10 }, (_, pair) => ({
        id: `pair-${pair}`,
        latexCaseId: `tex-${pair}`,
        markdownCaseId: `md-${pair}`,
      })),
      schemaVersion: 2,
    },
  };
}

function sourceCase(id: string, language: string, fileId: string) {
  return {
    id, language,
    namedNeedles: [
      { fileId, id: "formula", needle: "\nx=y\n", snapshotId: "current" },
      { fileId, id: "selection", needle: "x=y", parentAnchor: "formula", snapshotId: "current" },
    ],
    pairId: `pair-${id.split("-")[1]}`,
    selections: [{ anchor: "selection", id: "primary", snapshotId: "current" }],
    snapshots: [{
      dependencies: [],
      documents: [{ content: "A \nx=y\n relation.", fileId, path: fileId }],
      id: "current",
      mainFileId: fileId,
    }],
  };
}

function finalizedFixture(): FixtureValue {
  const fixture = fixtureValue();
  finalize(fixture);
  return fixture;
}

function dependencyFixture(): FixtureValue {
  const fixture = fixtureValue();
  for (const sourceCase of fixture.source.cases.slice(0, 2)) {
    const snapshot = sourceCase.snapshots[0]!;
    const main = snapshot.documents[0]!;
    const extension = sourceCase.language === "latex" ? ".tex" : ".md";
    const rolesFileId = `roles${extension}`;
    main.content = `${main.content} include roles`;
    snapshot.documents.push({ content: "roles edge", fileId: rolesFileId, path: rolesFileId });
    snapshot.dependencies.push({ fromFileId: main.fileId, sourceAnchor: "include-edge", toFileId: rolesFileId });
    sourceCase.namedNeedles.push(
      { fileId: main.fileId, id: "include-edge", needle: "include roles", snapshotId: "current" },
      { fileId: rolesFileId, id: "roles-edge", needle: "roles edge", snapshotId: "current" },
    );
  }
  finalize(fixture);
  return fixture;
}

function capFixture(weakAlternativeCount: number): FixtureValue {
  const fixture = fixtureValue();
  for (const caseId of ["tex-0", "md-0"]) {
    const item = fixture.oracle.cases.find((entry) => entry.id === caseId)!;
    const weak = Array.from({ length: weakAlternativeCount }, (_, index) => ({
      bindings: [{ parameter: "value", symbol: "x" }],
      conditions: [{ conditionId: "same-context", label: "Same context.", status: "verified" }],
      dependentFacets: ["interpretations"],
      evidence: [`evidence-${caseId}`],
      id: `weak-${index}`,
      releaseRequired: false,
      selector: {
        formulaAnchor: `${caseId}:formula`,
        kind: "structural-alternative",
        label: `Reviewed weak alternative ${index}`,
      },
      supportAllowed: ["tentative"],
    }));
    item.advisory.requiredHypotheses.push(...weak);
    item.cap = {
      correctHypothesisId: "main",
      exposedExact: 16,
      preCapRequiredHypotheses: ["main", ...weak.map((entry) => entry.id)].map((requiredHypothesisId) => ({
        formulaGeneration: "authored",
        formulaLifecycle: "current",
        requiredHypothesisId,
      })),
      requiredLimitKinds: ["candidate-set-capped"],
    };
    item.safety.interpretationsTruncated = true;
    item.safety.lifecycle.capped = true;
    item.safety.limits = [{ evidence: [`evidence-${caseId}`], kind: "candidate-set-capped" }];
    item.safety.truncated = true;
  }
  finalize(fixture);
  return fixture;
}

function cappedObservations(
  fixture: FixtureValue,
  compiled: ReturnType<typeof compileFixture>,
): MathAuthoringOracleObservation[] {
  return observations(fixture).map((observation) => {
    if (!observation.authoringContext || (observation.caseId !== "tex-0" && observation.caseId !== "md-0")) return observation;
    const context = observation.authoringContext;
    const main = context.interpretations.hypotheses[0]!;
    const reference = { evidence: main.evidence[0]!.evidence, sourceAnchors: main.evidence[0]!.sourceAnchors };
    const { relation: _relation, ...relationless } = main;
    const alternatives = Array.from({ length: 15 }, (_, index) => ({
      ...relationless,
      hypothesisId: `weak/${index}`,
      kind: "structural-alternative" as const,
      label: `Reviewed weak alternative ${index}`,
      rank: index + 1,
      support: "tentative" as const,
    }));
    return {
      ...observation,
      authoringContext: {
        ...context,
        interpretations: {
          ...context.interpretations,
          analysisLimits: [{ evidence: [reference], kind: "candidate-set-capped" }],
          candidateCap: {
            candidateCountBeforeCap: compiled.capExpectations[observation.caseId]!.candidateCountBeforeCap,
            preCapSemanticKeyDigest: compiled.capExpectations[observation.caseId]!.preCapSemanticKeyDigest,
          },
          hypotheses: [main, ...alternatives],
          truncated: true,
        },
        lifecycle: { ...context.lifecycle, capped: true },
        truncated: true,
      },
    };
  });
}

function finalize(fixture: FixtureValue): void {
  const source = parseMathAuthoringOracleSource(fixture.source);
  fixture.oracle.sourceFixtureId = source.fixtureId;
  fixture.oracle.sourceSha256 = mathAuthoringOracleSourceDigest(source);
  const unsigned = parseMathAuthoringOracle(fixture.oracle);
  fixture.oracle.review.attestationDigest = mathAuthoringOracleReviewAttestationDigest(
    reviewAttestation(unsigned),
  );
  const reviewed = parseMathAuthoringOracle(fixture.oracle);
  fixture.oracle.review.digest = mathAuthoringOracleReviewDigest(source, reviewed);
}

function compileFixture(fixture: FixtureValue) {
  const oracle = parseMathAuthoringOracle(fixture.oracle);
  return compileMathAuthoringOracle(fixture.source, fixture.oracle, reviewAttestation(oracle));
}

function reviewAttestation(oracle: MathAuthoringOracle): MathAuthoringOracleReviewAttestation {
  return {
    oracleConstraintSha256: mathAuthoringOracleConstraintDigest(oracle),
    reviewedAt: oracle.review.reviewedAt,
    reviewer: oracle.review.reviewer,
    schemaVersion: 2,
    sourceFixture: oracle.sourceFixture,
    sourceFixtureId: oracle.sourceFixtureId,
    sourceSha256: oracle.sourceSha256,
    verdict: "approved",
  };
}

function observations(fixture: FixtureValue): MathAuthoringOracleObservation[] {
  return fixture.source.cases.flatMap((item) => {
    const snapshotId = item.selections[0]!.snapshotId;
    const context = contextFor(item, snapshotId);
    const selection = selectionFor(item, snapshotId, "x=y");
    return [
      { authoringContext: context, caseId: item.id, mode: "clean" as const, selection, snapshotId },
      { authoringContext: structuredClone(context), caseId: item.id, mode: "incremental" as const, selection, snapshotId },
    ];
  });
}

function selectionFor(
  item: FixtureValue["source"]["cases"][number],
  snapshotId: string,
  needle: string,
): MathAuthoringOracleObservation["selection"] {
  const snapshot = item.snapshots.find((entry) => entry.id === snapshotId)!;
  const document = snapshot.documents[0]!;
  const startOffset = document.content.indexOf(needle);
  return {
    documentVersion: documentVersionAt(item, snapshotId, document.fileId),
    location: {
      fileId: document.fileId,
      path: document.path,
      range: { endOffset: startOffset + needle.length, startOffset },
    },
  };
}

function contextFor(
  item: FixtureValue["source"]["cases"][number],
  snapshotId: string,
): MathAuthoringContext {
  const snapshot = item.snapshots.find((entry) => entry.id === snapshotId)!;
  const document = snapshot.documents[0]!;
  const documentVersion = documentVersionAt(item, snapshotId, document.fileId);
  const selectionStart = document.content.indexOf("x=y");
  const range = { endOffset: selectionStart + 3, startOffset: selectionStart };
  const formulaRange = {
    endOffset: selectionStart + 4,
    startOffset: selectionStart - 1,
  };
  const location = { fileId: document.fileId, path: document.path, range };
  const formulaLocation = { fileId: document.fileId, path: document.path, range: formulaRange };
  const evidence = {
    kind: "source-structure", ruleId: "test/relation", sourceRanges: [formulaRange], strength: "hard",
  };
  const anchor = {
    documentVersion,
    generation: "authored" as const,
    lifecycle: "current" as const,
    location: formulaLocation,
    scopePath: [],
  };
  const formula = {
    documentVersion,
    location: formulaLocation, scopePath: [], sourceNotation: "\nx=y\n",
  };
  return {
    claimEvidence: [], conditions: [], disposition: "established", equationLinks: [], formula,
    lifecycle: {
      capped: false, documentVersion, editable: true,
      engineLimited: false, freshness: "current", generation: "authored", retracted: false,
    },
    interpretations: {
      analysisLimits: [], exhaustiveness: "bounded-open-world",
      hypotheses: [{
        bindings: [{ constraint: { kind: "scalar" }, evidence, parameter: "value", proof: "typed", symbol: "x" }],
        conditions: [{
          conditionId: "same-context", evidence: [evidence], kind: "same-context",
          label: "Same context.", status: "verified", subjects: ["value"],
        }], documentVersion,
        evidence: [{ evidence, provenance: "typed-structure", role: "supporting", sourceAnchors: [anchor] }],
        formula, hypothesisId: `hypothesis/${item.id}`, kind: "typed-law",
        label: "Reviewed relation", location, missingDiscriminatorIds: [],
        orderingReasons: [{ evidence: [{ evidence, sourceAnchors: [anchor] }], kind: "typed-evidence" }],
        range, rank: 0,
        relation: {
          conditions: [], description: "Reviewed relation", evidence: [evidence],
          range, relationId: "test/relation", roles: [], title: "Reviewed relation",
        },
        scopePath: [], support: "supported",
      }],
      missingDiscriminators: [], truncated: false,
    },
    notationOccurrences: [], requirements: [], truncated: false,
  };
}

function documentVersionAt(
  item: FixtureValue["source"]["cases"][number],
  snapshotId: string,
  fileId: string,
): number {
  let prior: { content: string; path: string } | undefined;
  let version = 0;
  for (const snapshot of item.snapshots) {
    const document = snapshot.documents.find((entry) => entry.fileId === fileId);
    if (document &&
      (!prior || prior.content !== document.content || prior.path !== document.path)) {
      version += 1;
    }
    if (document) prior = { content: document.content, path: document.path };
    if (snapshot.id === snapshotId) return version;
  }
  throw new Error(`unknown snapshot ${snapshotId}`);
}
