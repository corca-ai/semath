import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import type {
  CompiledMathAuthoringOracle,
  MathAuthoringExpectedObservation,
  MathAuthoringOracleReport,
} from "../packages/evaluation/src/index";
import type {
  AnalysisStats,
  ChangeEnvelope,
  MathAuthoringContext,
  ProjectSnapshot,
  QueryEnvelope,
  QueryResult,
  SemanticViewInfo,
  UpdateResult,
} from "../packages/protocol/src/index";
import {
  assertCleanIncrementalMathAuthoringParity,
  assertNativeWasmMathAuthoringParity,
  buildMathAuthoringProjectSnapshot,
  classifyMathAuthoringOracleReport,
  mathAuthoringObservation,
  mathAuthoringChangedFileIds,
  mathAuthoringProjectChanges,
  mathAuthoringQueryFor,
  mathAuthoringSelectionOffset,
  parseMathAuthoringQueryResult,
  parseMathAuthoringUpdateResult,
  persistMathAuthoringDiagnostic,
  runMathAuthoringOracleWithPorts,
  type MathAuthoringOracleRunnerPorts,
} from "./math-authoring-oracle-runner";

describe("math authoring oracle runner", () => {
  test("uses one declared structural edge for TeX and synthesizes the equivalent Markdown edge", () => {
    const tex = fixture("latex");
    const texProject = buildMathAuthoringProjectSnapshot(tex.compiled, "case", "current", "tex", 1);
    expect(texProject.documents.find((item) => item.fileId === "main")?.includes).toHaveLength(1);
    expect(texProject.documents.find((item) => item.fileId === "main")?.includes[0]?.path).toBe("roles");

    const markdown = fixture("markdown");
    const markdownProject = buildMathAuthoringProjectSnapshot(markdown.compiled, "case", "current", "md", 1);
    expect(markdownProject.documents.find((item) => item.fileId === "main")?.includes).toEqual([
      {
        path: "roles.md",
        source: {
          fileId: "main",
          path: "main.md",
          range: markdown.dependencyRange,
        },
        type: "input",
      },
    ]);
  });

  test("rejects unreviewed local includes", () => {
    const value = fixture("latex");
    const sourceCase = value.compiled.source.cases[0]!;
    const compiled = {
      ...value.compiled,
      source: {
        ...value.compiled.source,
        cases: [{
          ...sourceCase,
          snapshots: [{ ...sourceCase.snapshots[0]!, dependencies: [] }],
        }],
      },
    };
    expect(() => buildMathAuthoringProjectSnapshot(compiled, "case", "current", "bad", 1))
      .toThrow("undeclared authored dependency");
  });

  test("rejects declarations without a source surface and unresolved authored includes", () => {
    const markdown = fixture("markdown");
    const withoutLink = replaceProjectSource(
      markdown,
      "The roles are mentioned without a link.\n\n$$x=y$$\n",
    );
    expect(() => buildMathAuthoringProjectSnapshot(withoutLink, "case", "current", "missing-surface", 1))
      .toThrow("has no exact authored source surface");

    const tex = fixture("latex");
    const unresolved = replaceProjectSource(
      tex,
      "\\input{roles}\n\\input{missing}\n\\[x=y\\]\n",
    );
    expect(() => buildMathAuthoringProjectSnapshot(unresolved, "case", "current", "unresolved", 1))
      .toThrow("unresolved authored dependency missing");
  });

  test("requires the reviewed anchor to contain the exact target surface", () => {
    const value = fixture("markdown");
    const compiled = {
      ...value.compiled,
      anchors: {
        ...value.compiled.anchors,
        "case:dependency": {
          ...value.compiled.anchors["case:dependency"]!,
          location: {
            ...value.compiled.anchors["case:dependency"]!.location,
            range: value.formulaRange,
          },
        },
      },
    };
    expect(() => buildMathAuthoringProjectSnapshot(compiled, "case", "current", "wrong-anchor", 1))
      .toThrow("has no exact authored source surface");
  });

  test("does not use a commented singleton as the declared TeX source surface", () => {
    const value = fixture("latex");
    const content = "% \\input{roles}\n\\input{roles}\n\\[x=y\\]\n";
    const dependencyStart = content.lastIndexOf("roles");
    const formulaStart = content.indexOf("x=y");
    const compiled = replaceProjectSource(
      value,
      content,
      "main.tex",
      "roles.tex",
      { startOffset: dependencyStart, endOffset: dependencyStart + "roles".length },
      { startOffset: formulaStart, endOffset: formulaStart + 3 },
    );
    const project = buildMathAuthoringProjectSnapshot(compiled, "case", "current", "comment", 1);
    expect(project.documents.find((item) => item.fileId === "main")?.includes).toHaveLength(1);
    expect(project.documents.find((item) => item.fileId === "main")?.includes[0]?.source.range)
      .toEqual({ startOffset: dependencyStart, endOffset: dependencyStart + "roles".length });
  });

  test("writes synthesized dependency paths relative to a nested source document", () => {
    const value = fixture("markdown");
    const content = "[roles](../shared/roles.md)\n\n$$x=y$$\n";
    const dependencyStart = content.indexOf("../shared/roles.md");
    const formulaStart = content.indexOf("x=y");
    const compiled = replaceProjectSource(
      value,
      content,
      "sections/main.md",
      "shared/roles.md",
      { startOffset: dependencyStart, endOffset: dependencyStart + "../shared/roles.md".length },
      { startOffset: formulaStart, endOffset: formulaStart + 3 },
    );
    const project = buildMathAuthoringProjectSnapshot(compiled, "case", "current", "nested", 1);
    expect(project.documents.find((item) => item.fileId === "main")?.includes[0]?.path)
      .toBe("../shared/roles.md");
  });

  test("plans deterministic empty-to-incremental changes", () => {
    const value = fixture("markdown");
    const first = buildMathAuthoringProjectSnapshot(value.compiled, "case", "current", "delta", 1);
    expect(mathAuthoringProjectChanges(undefined, first).map((item) => item.kind)).toEqual([
      "upsert",
      "upsert",
    ]);
    const next = {
      ...first,
      documents: first.documents
        .filter((item) => item.fileId !== "roles")
        .map((item) => ({ ...item, content: `${item.content}\n`, documentVersion: 2 })),
      inventoryVersion: 2,
    };
    expect(mathAuthoringProjectChanges(first, next).map((item) =>
      item.kind === "upsert" ? `upsert:${item.document.fileId}` : `${item.kind}:${item.fileId}`
    )).toEqual(["remove:roles", "upsert:main"]);
  });

  test("compares update receipts in the engine's stable file-id order", () => {
    const value = fixture("markdown");
    const document = buildMathAuthoringProjectSnapshot(
      value.compiled,
      "case",
      "current",
      "ordered-receipt",
      1,
    ).documents[0]!;
    expect(mathAuthoringChangedFileIds([
      { fileId: "z-removed", kind: "remove" },
      { document: { ...document, fileId: "a-upserted" }, kind: "upsert" },
    ])).toEqual(["a-upserted", "z-removed"]);
  });

  test("resolves present math and removed prose selections without inventing context", () => {
    const value = fixture("markdown");
    const project = buildMathAuthoringProjectSnapshot(value.compiled, "case", "current", "select", 1);
    const document = project.documents.find((item) => item.fileId === "main")!;
    const present = expected(value, "present", "formula");
    expect(mathAuthoringSelectionOffset(document, present)).toBe(value.formulaRange.endOffset);
    const partiallyOverlapping = {
      ...present,
      selection: {
        ...present.selection,
        location: {
          ...present.selection.location,
          range: {
            startOffset: value.formulaRange.startOffset - 3,
            endOffset: value.formulaRange.endOffset,
          },
        },
      },
    };
    expect(() => mathAuthoringSelectionOffset(document, partiallyOverlapping))
      .toThrow("must be contained by exactly one math root");

    const removed = expected(value, "absent", "dependency");
    expect(mathAuthoringSelectionOffset(document, removed)).toBe(value.dependencyRange.endOffset);
    const empty = emptyContext();
    expect(mathAuthoringObservation(removed, view(empty)).authoringContext).toBeUndefined();
    expect(mathAuthoringObservation(removed, view({ ...empty, formula: {
      documentVersion: 1,
      location: { fileId: "main", path: "main.md", range: value.formulaRange },
      scopePath: [],
      sourceNotation: "x=y",
    } })).authoringContext).toBeDefined();
  });

  test("makes parity failures fatal and separates advisory gate output", () => {
    const context = emptyContext();
    expect(() => assertNativeWasmMathAuthoringParity(queryResult(context), queryResult(context), "same")).not.toThrow();
    expect(() => assertNativeWasmMathAuthoringParity(
      queryResult(context),
      queryResult({ ...context, disposition: "ambiguous" }),
      "different",
    )).toThrow("native/WASM");
    expect(() => assertCleanIncrementalMathAuthoringParity(context, context, "same"))
      .not.toThrow();

    const gate = classifyMathAuthoringOracleReport({
      advisoryFindings: ["missing optional suggestion"],
      diagnostic: { artifactId: `sha256:${"a".repeat(64)}`, sha256: "a".repeat(64) },
      pairFailures: ["formats differ"],
      safetyFailures: ["authority escaped"],
      suppressedFacets: ["conditions"],
      transitionFailures: ["stale anchor"],
    } satisfies MathAuthoringOracleReport);
    expect(gate.advisory).toEqual(["missing optional suggestion", "suppressed facet: conditions"]);
    expect(gate.blocking).toEqual([
      "safety: authority escaped",
      "pair: formats differ",
      "transition: stale anchor",
    ]);
  });

  test("orchestrates empty-to-snapshot clean/native/WASM/incremental receipts", () => {
    const value = fixture("markdown");
    const clean = expected(value, "present", "formula");
    const incremental = { ...clean, mode: "incremental" as const };
    const trace: string[] = [];
    const observations = runMathAuthoringOracleWithPorts(
      value.compiled,
      fakePorts(trace),
      [clean, incremental],
    );
    expect(observations.map((item) => item.mode)).toEqual(["clean", "incremental"]);
    expect(trace).toEqual([
      "engine:0:create", "engine:0:reset:0", "engine:0:apply:1",
      "native:1", "engine:1:create", "engine:1:reset:1", "engine:1:query:0",
      "engine:1:free", "engine:0:query:1", "engine:0:free",
    ]);
  });

  test("rejects query receipt mismatches before observation", () => {
    const value = fixture("markdown");
    const project = buildMathAuthoringProjectSnapshot(value.compiled, "case", "current", "receipt", 1);
    const query = queryEnvelope(project, expected(value, "present", "formula"), 0);
    expect(() => parseMathAuthoringQueryResult(
      {
        ...queryResult(emptyContext(), query),
        value: {
          ...queryResult(emptyContext(), query).value,
          view: {
            ...(queryResult(emptyContext(), query).value as { view: object }).view,
            symbol: null,
          },
        },
      },
      query,
      "receipt-null-symbol",
    )).not.toThrow();
    expect(() => parseMathAuthoringQueryResult(
      { ...queryResult(emptyContext(), query), inventoryVersion: 9 },
      query,
      "receipt",
    )).toThrow("query receipt mismatch");

    const trace: string[] = [];
    expect(() => runMathAuthoringOracleWithPorts(
      value.compiled,
      fakePorts(trace, { nativeReceiptMismatch: true }),
      [expected(value, "present", "formula"), { ...expected(value, "present", "formula"), mode: "incremental" }],
    )).toThrow("query receipt mismatch");
    expect(trace.at(-1)).toBe("engine:0:free");
  });

  test("strictly validates reset and apply update receipts", () => {
    const valid = updateResult("updates", 2, 2, ["main"], 1);
    const expectedReceipt = {
      analysisGeneration: 2,
      changedFileIds: ["main"],
      epoch: "updates",
      inventoryVersion: 2,
      totalDocuments: 1,
    };
    expect(() => parseMathAuthoringUpdateResult(valid, expectedReceipt)).not.toThrow();
    expect(() => parseMathAuthoringUpdateResult(
      { ...valid, changedFileIds: ["other"] },
      expectedReceipt,
    )).toThrow("update receipt mismatch");
    expect(() => parseMathAuthoringUpdateResult(
      { ...valid, surprise: true },
      expectedReceipt,
    )).toThrow("unexpected keys surprise");
  });

  test("frees both incremental and clean engines when a clean query throws", () => {
    const value = fixture("markdown");
    const trace: string[] = [];
    expect(() => runMathAuthoringOracleWithPorts(
      value.compiled,
      fakePorts(trace, { throwCleanQuery: true }),
      [expected(value, "present", "formula"), { ...expected(value, "present", "formula"), mode: "incremental" }],
    )).toThrow("clean query failed");
    expect(trace.filter((item) => item.endsWith(":free"))).toEqual([
      "engine:1:free",
      "engine:0:free",
    ]);
  });

  test("persists diagnostics only at their fixed content address and refuses different bytes", async () => {
    const content = "diagnostic\n";
    const digest = createHash("sha256").update(content).digest("hex");
    const files = new Map<string, string>();
    const directories: string[] = [];
    const io = {
      mkdir: async (path: string) => { directories.push(path); },
      read: async (path: string) => files.get(path),
      writeExclusive: async (path: string, content: string) => {
        if (files.has(path)) return false;
        files.set(path, content);
        return true;
      },
    };
    const artifact = { artifactId: `sha256:${digest}`, content, sha256: digest };
    const path = await persistMathAuthoringDiagnostic(artifact, io);
    expect(path).toBe(`.artifacts/math-authoring-oracle/${digest}.json`);
    expect(directories).toEqual([".artifacts/math-authoring-oracle"]);
    await expect(persistMathAuthoringDiagnostic(artifact, io)).resolves.toBe(path);
    files.set(path, "tampered\n");
    await expect(persistMathAuthoringDiagnostic(artifact, io)).rejects.toThrow("bytes differ");
    await expect(persistMathAuthoringDiagnostic({ ...artifact, content: "changed\n" }, io))
      .rejects.toThrow("content digest mismatch");
  });
});

function fixture(language: "latex" | "markdown") {
  const extension = language === "latex" ? "tex" : "md";
  const mainContent = language === "latex"
    ? "\\input{roles}\n\\[x=y\\]\n"
    : "[roles](roles.md)\n\n$$x=y$$\n";
  const dependencyNeedle = language === "latex" ? "roles" : "roles.md";
  const dependencyStart = mainContent.indexOf(dependencyNeedle);
  const formulaStart = mainContent.indexOf("x=y");
  const dependencyRange = {
    startOffset: dependencyStart,
    endOffset: dependencyStart + dependencyNeedle.length,
  };
  const formulaRange = { startOffset: formulaStart, endOffset: formulaStart + 3 };
  const sourceCase = {
    id: "case",
    language,
    namedNeedles: [],
    pairId: "pair",
    selections: [{ anchor: "formula", id: "primary", snapshotId: "current" }],
    snapshots: [{
      dependencies: [{ fromFileId: "main", sourceAnchor: "dependency", toFileId: "roles" }],
      documents: [
        { content: mainContent, documentVersion: 1, fileId: "main", path: `main.${extension}` },
        { content: "Let x and y be scalars.\n", documentVersion: 1, fileId: "roles", path: `roles.${extension}` },
      ],
      id: "current",
      mainFileId: "main",
    }],
  } as const;
  const compiled = {
    anchors: {
      "case:dependency": {
        caseId: "case", documentVersion: 1, fileId: "main",
        location: { fileId: "main", path: `main.${extension}`, range: dependencyRange },
        logicalId: "pair:dependency", snapshotId: "current",
      },
      "case:formula": {
        caseId: "case", documentVersion: 1, fileId: "main",
        location: { fileId: "main", path: `main.${extension}`, range: formulaRange },
        logicalId: "pair:formula", snapshotId: "current",
      },
    },
    capExpectations: {},
    oracle: { cases: [], evidence: {}, pairs: [] },
    source: { cases: [sourceCase], fixtureId: "fixture", pairs: [], schemaVersion: 2 },
  } as unknown as CompiledMathAuthoringOracle;
  return { compiled, dependencyRange, formulaRange };
}

function expected(
  value: ReturnType<typeof fixture>,
  context: "absent" | "present",
  anchor: "dependency" | "formula",
): MathAuthoringExpectedObservation {
  return {
    caseId: "case",
    context,
    mode: "clean",
    selection: value.compiled.anchors[`case:${anchor}`]!,
    selectionAnchorId: `case:${anchor}`,
    snapshotId: "current",
    sourceCaseId: "case",
  };
}

function replaceProjectSource(
  value: ReturnType<typeof fixture>,
  content: string,
  mainPath = value.compiled.source.cases[0]!.snapshots[0]!.documents[0]!.path,
  rolePath = value.compiled.source.cases[0]!.snapshots[0]!.documents[1]!.path,
  dependencyRange = value.dependencyRange,
  formulaRange = value.formulaRange,
): CompiledMathAuthoringOracle {
  const sourceCase = value.compiled.source.cases[0]!;
  const snapshot = sourceCase.snapshots[0]!;
  return {
    ...value.compiled,
    anchors: {
      ...value.compiled.anchors,
      "case:dependency": {
        ...value.compiled.anchors["case:dependency"]!,
        location: { fileId: "main", path: mainPath, range: dependencyRange },
      },
      "case:formula": {
        ...value.compiled.anchors["case:formula"]!,
        location: { fileId: "main", path: mainPath, range: formulaRange },
      },
    },
    source: {
      ...value.compiled.source,
      cases: [{
        ...sourceCase,
        snapshots: [{
          ...snapshot,
          documents: [
            { ...snapshot.documents[0]!, content, path: mainPath },
            { ...snapshot.documents[1]!, path: rolePath },
          ],
        }],
      }],
    },
  };
}

function emptyContext(): MathAuthoringContext {
  return {
    claimEvidence: [],
    conditions: [],
    disposition: "unsupported",
    equationLinks: [],
    interpretations: {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [],
      missingDiscriminators: [],
      truncated: false,
    },
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    },
    notationOccurrences: [],
    requirements: [],
    truncated: false,
  };
}

function view(authoringContext: MathAuthoringContext): SemanticViewInfo {
  return { authoringContext } as SemanticViewInfo;
}

function queryResult(
  authoringContext: MathAuthoringContext,
  query: QueryEnvelope = {
    analysisGeneration: 0,
    documentVersion: 1,
    epoch: "test",
    inventoryVersion: 1,
    protocolVersion: 17,
    query: { fileId: "main", kind: "semanticView", offset: 0 },
  },
): QueryResult {
  return {
    analysisGeneration: query.analysisGeneration,
    documentVersion: query.documentVersion,
    epoch: query.epoch,
    inventoryVersion: query.inventoryVersion,
    protocolVersion: query.protocolVersion,
    value: {
      kind: "semanticView",
      view: {
        authoringContext,
        context: {
          candidates: [], claims: [], concepts: [], quantities: [], relations: [], truncated: false,
        },
        decision: { reasons: [], status: "unsupported" },
        declarations: [],
        diagnostics: [],
        domains: [],
        truncated: false,
      },
    },
  };
}

function queryEnvelope(
  project: ProjectSnapshot,
  planned: MathAuthoringExpectedObservation,
  generation: number,
): QueryEnvelope {
  return mathAuthoringQueryFor(project, planned, generation);
}

function fakePorts(
  trace: string[],
  behavior: { readonly nativeReceiptMismatch?: boolean; readonly throwCleanQuery?: boolean } = {},
): MathAuthoringOracleRunnerPorts {
  let engineIndex = 0;
  return {
    createEngine: () => {
      const id = engineIndex++;
      const files = new Set<string>();
      let totalDocuments = 0;
      trace.push(`engine:${id}:create`);
      return {
        reset: (snapshot) => {
          trace.push(`engine:${id}:reset:${snapshot.inventoryVersion}`);
          files.clear();
          snapshot.documents.forEach((item) => files.add(item.fileId));
          totalDocuments = files.size;
          return updateResult(
            snapshot.epoch,
            snapshot.inventoryVersion,
            0,
            snapshot.documents.map((item) => item.fileId).sort(),
            totalDocuments,
          );
        },
        apply: (envelope) => {
          trace.push(`engine:${id}:apply:${envelope.inventoryVersion}`);
          for (const change of envelope.changes) {
            if (change.kind === "remove") files.delete(change.fileId);
            else if (change.kind === "upsert") files.add(change.document.fileId);
          }
          totalDocuments = files.size;
          return updateResult(
            envelope.epoch,
            envelope.inventoryVersion,
            envelope.analysisGeneration,
            envelope.changes.map((item) => item.kind === "upsert" ? item.document.fileId : item.fileId),
            totalDocuments,
          );
        },
        query: (query) => {
          trace.push(`engine:${id}:query:${query.analysisGeneration}`);
          if (behavior.throwCleanQuery && id === 1) throw new Error("clean query failed");
          return queryResult(emptyContext(), query);
        },
        free: () => { trace.push(`engine:${id}:free`); },
      };
    },
    runNative: (_snapshot, queries) => {
      trace.push(`native:${queries.length}`);
      return queries.map((query) => ({
        ...queryResult(emptyContext(), query),
        ...(behavior.nativeReceiptMismatch ? { inventoryVersion: query.inventoryVersion + 1 } : {}),
      }));
    },
  };
}

function updateResult(
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
  changedFileIds: readonly string[],
  totalDocuments: number,
): UpdateResult {
  return {
    analysisGeneration,
    analyzedFileIds: [],
    changedFileIds,
    epoch,
    inventoryVersion,
    protocolVersion: 17,
    stats: analysisStats(totalDocuments),
  };
}

function analysisStats(totalDocuments: number): AnalysisStats {
  return {
    analyzedDocuments: 0,
    constraints: 0,
    domainEvidence: 0,
    domainHypotheses: 0,
    equivalenceGuardChecks: 0,
    equivalenceStates: 0,
    invalidatedSemanticClaims: 0,
    lawRulesVisited: 0,
    packFrontierCandidates: 0,
    packLatentCandidates: 0,
    packLatentFallbacks: 0,
    proseClauses: 0,
    proseConstructionCandidates: 0,
    proseMatcherWork: 0,
    recognizedLaws: 0,
    semanticCandidates: 0,
    semanticClaims: 0,
    semanticConstraintTruncated: false,
    semanticConstraintWork: 0,
    semanticDependencyEdges: 0,
    semanticDerivedClaims: 0,
    semanticEntities: 0,
    semanticEvidence: 0,
    semanticNodes: 0,
    semanticOccurrences: 0,
    totalDocuments,
  };
}
