import { createHash } from "node:crypto";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { dirname, extname, posix } from "node:path";
import { LatexSyntaxService, type LatexInclude } from "wasmtex/syntax";
import {
  compareMathAuthoringContext,
  firstDifferentialFailure,
  isMathAuthoringRemovedContextSafelyAbsent,
  mathAuthoringDiagnosticArtifactPath,
  mathAuthoringExpectedObservationPlan,
  parseObservedMathAuthoringContext,
  projectMathAuthoringContext,
  type CompiledMathAuthoringOracle,
  type MathAuthoringExpectedObservation,
  type MathAuthoringOracleObservation,
  type MathAuthoringOracleReport,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type MathAuthoringContext,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
  type SemanticViewInfo,
  type UpdateResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";

export interface MathAuthoringDiagnosticArtifact {
  readonly artifactId: string;
  readonly content: string;
  readonly sha256: string;
}

export interface MathAuthoringGateClassification {
  readonly advisory: readonly string[];
  readonly blocking: readonly string[];
}

export interface MathAuthoringDiagnosticIo {
  readonly mkdir: (path: string) => Promise<void>;
  readonly read: (path: string) => Promise<string | undefined>;
  readonly writeExclusive: (path: string, content: string) => Promise<boolean>;
}

export interface MathAuthoringOracleEnginePort {
  readonly apply: (changes: ChangeEnvelope) => unknown;
  readonly free: () => void;
  readonly query: (query: QueryEnvelope) => unknown;
  readonly reset: (snapshot: ProjectSnapshot) => unknown;
}

export interface MathAuthoringOracleRunnerPorts {
  readonly createEngine: () => MathAuthoringOracleEnginePort;
  readonly runNative: (
    snapshot: ProjectSnapshot,
    queries: readonly QueryEnvelope[],
    label: string,
  ) => unknown;
}

export function runMathAuthoringOracleWithPorts(
  compiled: CompiledMathAuthoringOracle,
  ports: MathAuthoringOracleRunnerPorts,
  expectedPlan: readonly MathAuthoringExpectedObservation[] = mathAuthoringExpectedObservationPlan(compiled),
): readonly MathAuthoringOracleObservation[] {
  const bySourceCase = new Map<string, MathAuthoringExpectedObservation[]>();
  for (const item of expectedPlan) {
    const values = bySourceCase.get(item.sourceCaseId) ?? [];
    values.push(item);
    bySourceCase.set(item.sourceCaseId, values);
  }
  const observations: MathAuthoringOracleObservation[] = [];
  for (const sourceCase of compiled.source.cases) {
    const planned = bySourceCase.get(sourceCase.id);
    if (!planned?.length) continue;
    const epoch = `math-authoring-oracle-${sourceCase.id}`;
    const mainFileId = assertStableMathAuthoringMainFile(compiled, sourceCase.id);
    const incremental = ports.createEngine();
    try {
      const empty = {
        documents: [], epoch, inventoryVersion: 0, mainFileId, projectId: epoch,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      } satisfies ProjectSnapshot;
      parseMathAuthoringUpdateResult(incremental.reset(empty), {
        analysisGeneration: 0,
        changedFileIds: [],
        epoch,
        inventoryVersion: 0,
        totalDocuments: 0,
      });
      let previous: ProjectSnapshot | undefined;
      for (let index = 0; index < sourceCase.snapshots.length; index += 1) {
        const sourceSnapshot = sourceCase.snapshots[index]!;
        const inventoryVersion = index + 1;
        const project = buildMathAuthoringProjectSnapshot(
          compiled, sourceCase.id, sourceSnapshot.id, epoch, inventoryVersion,
        );
        const changes = mathAuthoringProjectChanges(previous, project);
        const changeEnvelope = {
          analysisGeneration: inventoryVersion,
          changes,
          epoch,
          inventoryVersion,
          protocolVersion: SEMATH_PROTOCOL_VERSION,
        } satisfies ChangeEnvelope;
        parseMathAuthoringUpdateResult(incremental.apply(changeEnvelope), {
          analysisGeneration: inventoryVersion,
          changedFileIds: mathAuthoringChangedFileIds(changes),
          epoch,
          inventoryVersion,
          totalDocuments: project.documents.length,
        });
        const atSnapshot = planned.filter((item) => item.snapshotId === sourceSnapshot.id);
        const cleanExpected = atSnapshot.filter((item) => item.mode === "clean");
        const incrementalExpected = atSnapshot.filter((item) => item.mode === "incremental");
        const cleanResults = new Map<string, QueryResult>();
        if (cleanExpected.length) {
          const queries = cleanExpected.map((item) => mathAuthoringQueryFor(project, item, 0));
          const native = parseNativeQueryResults(
            ports.runNative(project, queries, `${sourceCase.id}:${sourceSnapshot.id}`),
            queries,
            `${sourceCase.id}:${sourceSnapshot.id}:native`,
          );
          const cleanEngine = ports.createEngine();
          try {
            parseMathAuthoringUpdateResult(cleanEngine.reset(project), {
              analysisGeneration: 0,
              changedFileIds: project.documents.map((item) => item.fileId).sort(),
              epoch,
              inventoryVersion,
              totalDocuments: project.documents.length,
            });
            for (let queryIndex = 0; queryIndex < queries.length; queryIndex += 1) {
              const query = queries[queryIndex]!;
              const item = cleanExpected[queryIndex]!;
              const wasm = parseMathAuthoringQueryResult(
                cleanEngine.query(query), query, `${item.caseId}:${item.snapshotId}:clean-wasm`,
              );
              assertNativeWasmMathAuthoringParity(native[queryIndex]!, wasm, `${item.caseId}:${item.snapshotId}`);
              cleanResults.set(observationKey(item), native[queryIndex]!);
              observations.push(mathAuthoringObservation(item, semanticView(native[queryIndex]!, "clean")));
            }
          } finally {
            cleanEngine.free();
          }
        }
        for (const item of incrementalExpected) {
          const query = mathAuthoringQueryFor(project, item, inventoryVersion);
          const result = parseMathAuthoringQueryResult(
            incremental.query(query), query, `${item.caseId}:${item.snapshotId}:incremental`,
          );
          const clean = required(cleanResults.get(observationKey(item)), `${item.caseId}:${item.snapshotId}: clean result missing`);
          assertCleanIncrementalMathAuthoringParity(
            semanticView(clean, "clean").authoringContext,
            semanticView(result, "incremental").authoringContext,
            `${item.caseId}:${item.snapshotId}`,
          );
          assertQueryValueParity(clean, result, `${item.caseId}:${item.snapshotId}:clean/incremental`);
          observations.push(mathAuthoringObservation(item, semanticView(result, "incremental")));
        }
        previous = project;
      }
    } finally {
      incremental.free();
    }
  }
  return observations;
}

export function buildMathAuthoringProjectSnapshot(
  compiled: CompiledMathAuthoringOracle,
  sourceCaseId: string,
  snapshotId: string,
  epoch: string,
  inventoryVersion: number,
): ProjectSnapshot {
  const sourceCase = required(
    compiled.source.cases.find((item) => item.id === sourceCaseId),
    `${sourceCaseId}: unknown source case`,
  );
  const snapshot = required(
    sourceCase.snapshots.find((item) => item.id === snapshotId),
    `${sourceCaseId}:${snapshotId}: unknown snapshot`,
  );
  const syntax = new LatexSyntaxService();
  syntax.reset({
    documents: snapshot.documents.map((document) => ({
      ...document,
      language: sourceCase.language,
    })),
  });
  const byFileId = new Map(snapshot.documents.map((document) => [document.fileId, document]));
  const documents = snapshot.documents.map((source): ProjectDocument => {
    const parsed = syntax.getFile(source.fileId);
    if (!parsed) throw new Error(`${sourceCaseId}:${snapshotId}:${source.fileId}: missing neutral syntax`);
    const adapted = adaptWasmtexDocument({
      content: source.content,
      language: sourceCase.language,
      syntax: parsed,
    });
    const declared = snapshot.dependencies.filter((item) => item.fromFileId === source.fileId);
    const surfaces = authoredDependencySurfaces(
      source.content,
      sourceCase.language,
      adapted.includes,
      `${sourceCaseId}:${snapshotId}:${source.fileId}`,
    );
    const unmatchedSurfaces = new Set(surfaces);
    const includes: LatexInclude[] = [];
    for (const dependency of declared) {
      const anchor = dependencyAnchor(
        compiled,
        sourceCaseId,
        snapshotId,
        source.fileId,
        dependency.sourceAnchor,
      );
      const target = required(
        byFileId.get(dependency.toFileId),
        `${sourceCaseId}:${snapshotId}: dependency target ${dependency.toFileId} missing`,
      );
      const surface = surfaces.find((candidate) =>
        unmatchedSurfaces.has(candidate) &&
        resolveIncludedFileId(source.path, candidate.path, byFileId) === dependency.toFileId &&
        rangeContains(anchor.location.range, candidate.targetRange)
      );
      if (!surface) {
        throw new Error(`${sourceCaseId}:${snapshotId}:${source.fileId}: declared dependency ${dependency.sourceAnchor} has no exact authored source surface`);
      }
      unmatchedSurfaces.delete(surface);
      includes.push({
        path: sourceCase.language === "markdown"
          ? relativeProjectPath(source.path, target.path)
          : surface.path,
        source: {
          fileId: source.fileId,
          path: source.path,
          range: anchor.location.range,
        },
        type: surface.type,
      });
    }
    for (const surface of unmatchedSurfaces) {
      const targetFileId = resolveIncludedFileId(source.path, surface.path, byFileId);
      if (targetFileId === undefined) {
        throw new Error(`${sourceCaseId}:${snapshotId}:${source.fileId}: unresolved authored dependency ${surface.path}`);
      }
      throw new Error(`${sourceCaseId}:${snapshotId}:${source.fileId}: undeclared authored dependency ${surface.path}`);
    }
    includes.sort((left, right) =>
      left.source.range.startOffset - right.source.range.startOffset ||
      left.path.localeCompare(right.path)
    );
    return { ...adapted, includes };
  });
  return {
    documents,
    epoch,
    inventoryVersion,
    mainFileId: snapshot.mainFileId,
    projectId: epoch,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

export function assertStableMathAuthoringMainFile(
  compiled: CompiledMathAuthoringOracle,
  sourceCaseId: string,
): string {
  const sourceCase = required(
    compiled.source.cases.find((item) => item.id === sourceCaseId),
    `${sourceCaseId}: unknown source case`,
  );
  const values = new Set(sourceCase.snapshots.map((item) => item.mainFileId));
  if (values.size !== 1) {
    throw new Error(`${sourceCaseId}: incremental history requires one stable mainFileId`);
  }
  return required(values.values().next().value, `${sourceCaseId}: source case has no snapshots`);
}

export function mathAuthoringProjectChanges(
  previous: ProjectSnapshot | undefined,
  next: ProjectSnapshot,
): ChangeEnvelope["changes"] {
  const before = new Map(previous?.documents.map((item) => [item.fileId, item]) ?? []);
  const after = new Map(next.documents.map((item) => [item.fileId, item]));
  const removed = [...before.keys()]
    .filter((fileId) => !after.has(fileId))
    .sort()
    .map((fileId) => ({ fileId, kind: "remove" as const }));
  const upserted = [...after.values()]
    .filter((document) => JSON.stringify(before.get(document.fileId)) !== JSON.stringify(document))
    .sort((left, right) => left.fileId.localeCompare(right.fileId))
    .map((document) => ({ document, kind: "upsert" as const }));
  return [...removed, ...upserted];
}

export function mathAuthoringChangedFileIds(
  changes: ChangeEnvelope["changes"],
): readonly string[] {
  return changes.map(changeFileId).sort();
}

export function mathAuthoringQueryFor(
  project: ProjectSnapshot,
  expected: MathAuthoringExpectedObservation,
  analysisGeneration: number,
): QueryEnvelope {
  const document = required(
    project.documents.find((item) => item.fileId === expected.selection.fileId),
    `${expected.caseId}:${expected.snapshotId}: selection file missing`,
  );
  return {
    analysisGeneration,
    documentVersion: expected.selection.documentVersion,
    epoch: project.epoch,
    inventoryVersion: project.inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: {
      fileId: expected.selection.fileId,
      kind: "semanticView",
      offset: mathAuthoringSelectionOffset(document, expected),
    },
  };
}

export function mathAuthoringSelectionOffset(
  document: ProjectDocument,
  expected: MathAuthoringExpectedObservation,
): number {
  if (document.fileId !== expected.selection.fileId ||
    document.path !== expected.selection.location.path ||
    document.documentVersion !== expected.selection.documentVersion) {
    throw new Error(`${expected.caseId}:${expected.snapshotId}:${expected.mode}: selection document receipt mismatch`);
  }
  const range = expected.selection.location.range;
  if (expected.context === "absent") {
    if (document.mathRoots.some((root) => rangesOverlap(root.contentRange, range))) {
      throw new Error(`${expected.caseId}:${expected.snapshotId}: absent selection overlaps math`);
    }
    return range.endOffset;
  }
  const roots = document.mathRoots.filter((root) => rangeContains(root.contentRange, range));
  if (roots.length !== 1) {
    throw new Error(`${expected.caseId}:${expected.snapshotId}: present selection must be contained by exactly one math root`);
  }
  return Math.min(range.endOffset, roots[0]!.contentRange.endOffset);
}

export function mathAuthoringObservation(
  expected: MathAuthoringExpectedObservation,
  view: SemanticViewInfo,
): MathAuthoringOracleObservation {
  const absent = expected.context === "absent" &&
    isMathAuthoringRemovedContextSafelyAbsent(view.authoringContext);
  return {
    ...(!absent ? { authoringContext: view.authoringContext } : {}),
    caseId: expected.caseId,
    mode: expected.mode,
    selection: {
      documentVersion: expected.selection.documentVersion,
      location: expected.selection.location,
    },
    snapshotId: expected.snapshotId,
  };
}

export function assertNativeWasmMathAuthoringParity(
  native: QueryResult,
  wasm: QueryResult,
  label: string,
): void {
  assertDifferentialParity(native, wasm, `${label}: native/WASM query result`);
}

export function assertCleanIncrementalMathAuthoringParity(
  clean: MathAuthoringContext,
  incremental: MathAuthoringContext,
  label: string,
): void {
  const failures = compareMathAuthoringContext(
    projectMathAuthoringContext(clean),
    incremental,
  );
  if (failures.length) {
    throw new Error(`${label}: clean/incremental authoring mismatch: ${failures.map((item) => `${item.path} ${item.kind}`).join(", ")}`);
  }
}

export function parseMathAuthoringQueryResult(
  value: unknown,
  query: QueryEnvelope,
  label: string,
): QueryResult {
  const result = exactRecord(value, label, [
    "analysisGeneration", "documentVersion", "epoch", "inventoryVersion", "protocolVersion", "value",
  ]);
  const receipt = {
    analysisGeneration: integer(result.analysisGeneration, `${label}.analysisGeneration`),
    documentVersion: positiveInteger(result.documentVersion, `${label}.documentVersion`),
    epoch: string(result.epoch, `${label}.epoch`),
    inventoryVersion: nonnegativeInteger(result.inventoryVersion, `${label}.inventoryVersion`),
    protocolVersion: integer(result.protocolVersion, `${label}.protocolVersion`),
  };
  assertDifferentialParity(
    {
      analysisGeneration: query.analysisGeneration,
      documentVersion: query.documentVersion,
      epoch: query.epoch,
      inventoryVersion: query.inventoryVersion,
      protocolVersion: query.protocolVersion,
    },
    receipt,
    `${label}: query receipt`,
  );
  const resultValue = exactRecord(result.value, `${label}.value`, ["kind", "view"]);
  if (resultValue.kind !== "semanticView") throw new Error(`${label}.value.kind: expected semanticView`);
  const view = exactRecord(resultValue.view, `${label}.value.view`, [
    "authoringContext", "context", "decision", "declarations", "diagnostics", "domains", "truncated",
  ], ["symbol"]);
  parseObservedMathAuthoringContext(view.authoringContext, `${label}.value.view.authoringContext`);
  record(view.context, `${label}.value.view.context`);
  record(view.decision, `${label}.value.view.decision`);
  array(view.declarations, `${label}.value.view.declarations`);
  array(view.diagnostics, `${label}.value.view.diagnostics`);
  array(view.domains, `${label}.value.view.domains`);
  boolean(view.truncated, `${label}.value.view.truncated`);
  if (view.symbol !== undefined && view.symbol !== null) {
    record(view.symbol, `${label}.value.view.symbol`);
  }
  return value as QueryResult;
}

export function parseMathAuthoringUpdateResult(
  value: unknown,
  expected: {
    readonly analysisGeneration: number;
    readonly changedFileIds: readonly string[];
    readonly epoch: string;
    readonly inventoryVersion: number;
    readonly totalDocuments: number;
  },
): UpdateResult {
  const label = `update:${expected.epoch}:${expected.inventoryVersion}`;
  const result = exactRecord(value, label, [
    "analysisGeneration", "analyzedFileIds", "changedFileIds", "epoch", "inventoryVersion", "protocolVersion", "stats",
  ]);
  const analyzedFileIds = stringArray(result.analyzedFileIds, `${label}.analyzedFileIds`);
  const changedFileIds = stringArray(result.changedFileIds, `${label}.changedFileIds`);
  const stats = exactRecord(result.stats, `${label}.stats`, analysisStatKeys);
  for (const key of analysisStatKeys) {
    if (key === "semanticConstraintTruncated") boolean(stats[key], `${label}.stats.${key}`);
    else nonnegativeInteger(stats[key], `${label}.stats.${key}`);
  }
  const receipt = {
    analysisGeneration: nonnegativeInteger(result.analysisGeneration, `${label}.analysisGeneration`),
    changedFileIds,
    epoch: string(result.epoch, `${label}.epoch`),
    inventoryVersion: nonnegativeInteger(result.inventoryVersion, `${label}.inventoryVersion`),
    protocolVersion: integer(result.protocolVersion, `${label}.protocolVersion`),
    totalDocuments: nonnegativeInteger(stats.totalDocuments, `${label}.stats.totalDocuments`),
  };
  assertDifferentialParity(
    { ...expected, protocolVersion: SEMATH_PROTOCOL_VERSION },
    receipt,
    `${label}: update receipt`,
  );
  if (new Set(analyzedFileIds).size !== analyzedFileIds.length) {
    throw new Error(`${label}.analyzedFileIds: duplicate file receipt`);
  }
  return value as UpdateResult;
}

export function classifyMathAuthoringOracleReport(
  report: MathAuthoringOracleReport,
): MathAuthoringGateClassification {
  return {
    advisory: [
      ...report.advisoryFindings,
      ...report.suppressedFacets.map((facet) => `suppressed facet: ${facet}`),
    ],
    blocking: [
      ...report.safetyFailures.map((item) => `safety: ${item}`),
      ...report.pairFailures.map((item) => `pair: ${item}`),
      ...report.transitionFailures.map((item) => `transition: ${item}`),
    ],
  };
}

export async function persistMathAuthoringDiagnostic(
  artifact: MathAuthoringDiagnosticArtifact,
  io: MathAuthoringDiagnosticIo = filesystemDiagnosticIo,
): Promise<string> {
  const contentDigest = createHash("sha256").update(artifact.content).digest("hex");
  if (contentDigest !== artifact.sha256) {
    throw new Error("math authoring diagnostic content digest mismatch");
  }
  const path = mathAuthoringDiagnosticArtifactPath(artifact);
  await io.mkdir(dirname(path));
  const existing = await io.read(path);
  if (existing !== undefined) {
    if (existing !== artifact.content) throw new Error(`${path}: content-addressed diagnostic bytes differ`);
    return path;
  }
  if (!await io.writeExclusive(path, artifact.content)) {
    const raced = await io.read(path);
    if (raced !== artifact.content) throw new Error(`${path}: content-addressed diagnostic write raced with different bytes`);
  }
  return path;
}

const analysisStatKeys = [
  "analyzedDocuments", "constraints", "lawRulesVisited", "packFrontierCandidates",
  "packLatentCandidates", "packLatentFallbacks", "domainHypotheses", "domainEvidence",
  "equivalenceStates", "equivalenceGuardChecks", "recognizedLaws", "semanticNodes",
  "semanticOccurrences", "semanticEntities", "semanticClaims", "semanticEvidence",
  "semanticDependencyEdges", "invalidatedSemanticClaims", "semanticCandidates",
  "semanticConstraintWork", "semanticDerivedClaims", "semanticConstraintTruncated",
  "proseClauses", "proseConstructionCandidates", "proseMatcherWork", "totalDocuments",
] as const;

const filesystemDiagnosticIo: MathAuthoringDiagnosticIo = {
  mkdir: async (path) => { await mkdir(path, { recursive: true }); },
  read: async (path) => {
    try {
      return await readFile(path, "utf8");
    } catch (error) {
      if (isNodeError(error, "ENOENT")) return undefined;
      throw error;
    }
  },
  writeExclusive: async (path, content) => {
    try {
      await writeFile(path, content, { encoding: "utf8", flag: "wx" });
      return true;
    } catch (error) {
      if (isNodeError(error, "EEXIST")) return false;
      throw error;
    }
  },
};

function dependencyAnchor(
  compiled: CompiledMathAuthoringOracle,
  sourceCaseId: string,
  snapshotId: string,
  fileId: string,
  anchorId: string,
) {
  const anchor = required(
    compiled.anchors[`${sourceCaseId}:${anchorId}`],
    `${sourceCaseId}:${snapshotId}: dependency anchor ${anchorId} missing`,
  );
  if (anchor.snapshotId !== snapshotId || anchor.fileId !== fileId) {
    throw new Error(`${sourceCaseId}:${snapshotId}: dependency anchor ${anchorId} receipt mismatch`);
  }
  return anchor;
}

function parseNativeQueryResults(
  value: unknown,
  queries: readonly QueryEnvelope[],
  label: string,
): readonly QueryResult[] {
  const values = array(value, label);
  if (values.length !== queries.length) throw new Error(`${label}: query count ${values.length}/${queries.length}`);
  return values.map((item, index) =>
    parseMathAuthoringQueryResult(item, queries[index]!, `${label}[${index}]`)
  );
}

function assertQueryValueParity(left: QueryResult, right: QueryResult, label: string): void {
  assertDifferentialParity(left.value, right.value, label);
}

function assertDifferentialParity(left: unknown, right: unknown, label: string): void {
  const failure = firstDifferentialFailure([
    { name: "native", value: left },
    { name: "wasm", value: right },
  ]);
  if (failure) {
    throw new Error(`${label} mismatch at ${failure.stage}:${failure.path}; expected=${JSON.stringify(failure.expected)} actual=${JSON.stringify(failure.actual)}`);
  }
}

function semanticView(result: QueryResult, label: string): SemanticViewInfo {
  if (result.value.kind !== "semanticView") throw new Error(`${label}: semantic view unavailable`);
  return result.value.view;
}

function observationKey(value: MathAuthoringExpectedObservation): string {
  return `${value.caseId}:${value.snapshotId}`;
}

function changeFileId(change: ChangeEnvelope["changes"][number]): string {
  return change.kind === "upsert" ? change.document.fileId : change.fileId;
}

function resolveIncludedFileId(
  fromPath: string,
  includePath: string,
  documents: ReadonlyMap<string, { readonly fileId: string; readonly path: string }>,
): string | undefined {
  const base = posix.normalize(posix.join(posix.dirname(fromPath), includePath));
  const candidates = extname(base) ? [base] : [base, `${base}.tex`, `${base}.md`];
  return [...documents.values()].find((item) => candidates.includes(posix.normalize(item.path)))?.fileId;
}

interface AuthoredDependencySurface {
  readonly commandRange: { readonly startOffset: number; readonly endOffset: number };
  readonly path: string;
  readonly targetRange: { readonly startOffset: number; readonly endOffset: number };
  readonly type: LatexInclude["type"];
}

function authoredDependencySurfaces(
  content: string,
  language: "latex" | "markdown",
  syntaxIncludes: readonly LatexInclude[],
  label: string,
): readonly AuthoredDependencySurface[] {
  if (language === "markdown") return markdownDependencySurfaces(content);
  const parsed = texDependencySurfaces(content);
  const remaining = [...parsed];
  const matched: AuthoredDependencySurface[] = [];
  for (const include of syntaxIncludes) {
    const index = remaining.findIndex((surface) =>
      surface.path === include.path && surface.type === include.type &&
      rangesOverlap(surface.commandRange, include.source.range)
    );
    if (index < 0) throw new Error(`${label}: syntax include ${include.path} has no exact authored source surface`);
    matched.push(remaining[index]!);
    remaining.splice(index, 1);
  }
  return matched;
}

function texDependencySurfaces(content: string): readonly AuthoredDependencySurface[] {
  const surfaces: AuthoredDependencySurface[] = [];
  const pattern = /\\(input|include|subfile)\s*\{([^{}]+)\}/gu;
  for (const match of content.matchAll(pattern)) {
    const command = match[1];
    const path = match[2];
    const start = match.index + match[0].lastIndexOf(path!);
    surfaces.push({
      commandRange: { startOffset: match.index, endOffset: match.index + match[0].length },
      path: path!,
      targetRange: { startOffset: start, endOffset: start + path!.length },
      type: command as LatexInclude["type"],
    });
  }
  return surfaces;
}

function markdownDependencySurfaces(content: string): readonly AuthoredDependencySurface[] {
  const surfaces: AuthoredDependencySurface[] = [];
  const pattern = /\[[^\]\n]*\]\(([^)\s]+)\)/gu;
  for (const match of content.matchAll(pattern)) {
    const path = match[1]!;
    const start = match.index + match[0].lastIndexOf(path);
    surfaces.push({
      commandRange: { startOffset: match.index, endOffset: match.index + match[0].length },
      path,
      targetRange: { startOffset: start, endOffset: start + path.length },
      type: "input",
    });
  }
  return surfaces;
}

function relativeProjectPath(fromPath: string, targetPath: string): string {
  const relative = posix.relative(posix.dirname(fromPath), targetPath);
  if (!relative || relative.startsWith("/")) throw new Error(`invalid project dependency path ${targetPath}`);
  return relative;
}

function rangeContains(
  container: { readonly startOffset: number; readonly endOffset: number },
  child: { readonly startOffset: number; readonly endOffset: number },
): boolean {
  return container.startOffset <= child.startOffset && child.endOffset <= container.endOffset;
}

function rangesOverlap(
  left: { readonly startOffset: number; readonly endOffset: number },
  right: { readonly startOffset: number; readonly endOffset: number },
): boolean {
  return left.startOffset < right.endOffset && right.startOffset < left.endOffset;
}

function exactRecord(
  value: unknown,
  path: string,
  requiredKeys: readonly string[],
  optionalKeys: readonly string[] = [],
): Readonly<Record<string, unknown>> {
  const item = record(value, path);
  const allowed = new Set([...requiredKeys, ...optionalKeys]);
  const missing = requiredKeys.filter((key) => !(key in item));
  const extra = Object.keys(item).filter((key) => !allowed.has(key));
  if (missing.length) throw new Error(`${path}: missing keys ${missing.join(", ")}`);
  if (extra.length) throw new Error(`${path}: unexpected keys ${extra.sort().join(", ")}`);
  return item;
}

function record(value: unknown, path: string): Readonly<Record<string, unknown>> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${path}: expected object`);
  return value as Readonly<Record<string, unknown>>;
}

function array(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path}: expected array`);
  return value;
}

function string(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.length) throw new Error(`${path}: expected non-empty string`);
  return value;
}

function stringArray(value: unknown, path: string): readonly string[] {
  return array(value, path).map((item, index) => string(item, `${path}[${index}]`));
}

function integer(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw new Error(`${path}: expected safe integer`);
  return value;
}

function nonnegativeInteger(value: unknown, path: string): number {
  const result = integer(value, path);
  if (result < 0) throw new Error(`${path}: expected nonnegative integer`);
  return result;
}

function positiveInteger(value: unknown, path: string): number {
  const result = integer(value, path);
  if (result <= 0) throw new Error(`${path}: expected positive integer`);
  return result;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path}: expected boolean`);
  return value;
}

function isNodeError(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && (error as NodeJS.ErrnoException).code === code;
}

function required<T>(value: T | undefined, message: string): T {
  if (value === undefined) throw new Error(message);
  return value;
}
