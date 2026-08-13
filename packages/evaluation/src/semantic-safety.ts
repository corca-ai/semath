import type { QueryResult, SourceRange } from "../../protocol/src/index";
import {
  normalizeSymbol,
  roleInstancesMatch,
  type ObservedRole,
} from "./observation";
import type { CorpusDocument } from "./model";

export const SEMANTIC_SAFETY_CONTRACTS = [
  "exact-establishment",
  "explicit-opposition",
  "calm-unsupported",
  "calm-ambiguity",
  "cross-scope-isolation",
  "navigation-complete",
  "navigation-reject",
  "lifecycle-retraction",
] as const;

export const SEMANTIC_SAFETY_TRANSFORMS = [
  "neutral-prefix",
  "trailing-comment",
  "document-order",
  "opposition-order",
] as const;

export type SemanticSafetyContract =
  (typeof SEMANTIC_SAFETY_CONTRACTS)[number];
export type SemanticSafetyTransform =
  (typeof SEMANTIC_SAFETY_TRANSFORMS)[number];
export type SemanticSafetyDecision =
  | "ambiguous"
  | "conflicting"
  | "established"
  | "partial"
  | "unsupported";

export interface SemanticSafetyLawCatalogEntry {
  readonly lawId: string;
  readonly roles: readonly string[];
}

export interface SemanticSafetyAnchor {
  readonly fileId: string;
  readonly needle: string;
  readonly occurrence?: number;
  readonly selection?: {
    readonly length: number;
    readonly offset: number;
  };
}

export interface SemanticSafetyCursor {
  readonly edge?: "after" | "before";
  readonly fileId: string;
  readonly needle: string;
  readonly occurrence?: number;
  readonly offset?: number;
}

interface SemanticSafetyRelationExpectation {
  readonly relationId: string;
  readonly roles: readonly ObservedRole[];
  readonly sourceGrounded: boolean;
}

export type SemanticSafetyNavigationExpectation =
  | { readonly mode: "skip" }
  | { readonly mode: "reject" }
  | {
      readonly definition: readonly SemanticSafetyAnchor[];
      readonly expectedText: string;
      readonly mode: "exact";
      readonly newName: string;
      readonly placeholder: string;
      readonly references: readonly SemanticSafetyAnchor[];
      readonly rename: readonly SemanticSafetyAnchor[];
      readonly replacementText: string;
      readonly safety: "deterministic" | "review-required";
    };

export interface SemanticSafetyExpectation {
  readonly decision: SemanticSafetyDecision;
  readonly excludedRelationIds: readonly string[];
  readonly maximumProblems: number;
  readonly navigation: SemanticSafetyNavigationExpectation;
  readonly proofGrounded: boolean;
  readonly relations: readonly SemanticSafetyRelationExpectation[];
}

export interface SemanticSafetyProbe {
  readonly expected: SemanticSafetyExpectation;
  readonly id: string;
  readonly navigationCursor?: SemanticSafetyCursor;
  readonly semanticCursor: SemanticSafetyCursor;
  readonly snapshotId: string;
}

export interface SemanticSafetySnapshot {
  readonly documents: readonly CorpusDocument[];
  readonly id: string;
}

export interface SemanticSafetyTransition {
  readonly fromProbeId: string;
  readonly kind: "retract-relation";
  readonly relationId: string;
  readonly toProbeId: string;
}

export interface SemanticSafetyCase {
  readonly contract: SemanticSafetyContract;
  readonly id: string;
  readonly lawIds: readonly string[];
  readonly pairId: string;
  readonly probes: readonly SemanticSafetyProbe[];
  readonly snapshots: readonly SemanticSafetySnapshot[];
  readonly transforms: readonly SemanticSafetyTransform[];
  readonly transitions: readonly SemanticSafetyTransition[];
  readonly variationTags: readonly string[];
}

export interface SemanticSafetySpec {
  readonly cases: readonly SemanticSafetyCase[];
  readonly id: string;
  readonly schemaVersion: 1;
}

export interface PlannedSemanticSafetyCase {
  readonly contract: SemanticSafetyContract;
  readonly documents: readonly CorpusDocument[];
  readonly expected: SemanticSafetyExpectation;
  readonly id: string;
  readonly navigationCursor?: SemanticSafetyCursor;
  readonly pairId: string;
  readonly probeId: string;
  readonly semanticCursor: SemanticSafetyCursor;
  readonly snapshotId: string;
  readonly sourceCaseId: string;
  readonly transform: "identity" | SemanticSafetyTransform;
}

export interface SemanticSafetyObservedRelation {
  readonly relationId: string;
  readonly roles: readonly ObservedRole[];
  readonly sourceGrounded: boolean;
}

export interface SemanticSafetyObservedLocation {
  readonly fileId: string;
  readonly path: string;
  readonly range: SourceRange;
}

export interface SemanticSafetyObservation {
  readonly caseId: string;
  readonly decision: SemanticSafetyDecision;
  readonly definitions: readonly SemanticSafetyObservedLocation[];
  readonly prepareRename: {
    readonly fileId?: string;
    readonly path?: string;
    readonly placeholder?: string;
    readonly range?: SourceRange;
  };
  readonly problemCodes: readonly string[];
  readonly meaningRelationId: string | null;
  readonly proofGrounded: boolean;
  readonly references: readonly SemanticSafetyObservedLocation[];
  readonly relations: readonly SemanticSafetyObservedRelation[];
  readonly rename: {
    readonly edits: readonly (SemanticSafetyObservedLocation & {
      readonly expectedText: string;
      readonly replacementText: string;
    })[];
    readonly safety?: "deterministic" | "review-required";
  };
}

export interface SemanticSafetyScorecard {
  readonly cases: number;
  readonly contractFailures: readonly string[];
  readonly failures: readonly string[];
  readonly passed: number;
  readonly safetyFailures: readonly string[];
}

export interface SemanticSafetySurfaceResults {
  readonly definition?: QueryResult;
  readonly diagnostics: QueryResult;
  readonly prepareRename?: QueryResult;
  readonly references?: QueryResult;
  readonly rename?: QueryResult;
  readonly semanticView: QueryResult;
}

export function parseSemanticSafetySpec(
  value: unknown,
  lawCatalog: readonly SemanticSafetyLawCatalogEntry[],
): SemanticSafetySpec {
  const root = record(value, "semantic safety spec");
  exact(root, ["schemaVersion", "id", "cases"], "semantic safety spec");
  if (root.schemaVersion !== 1) {
    throw new Error("semantic safety spec.schemaVersion: must be 1");
  }
  const laws = new Map(lawCatalog.map((law) => [law.lawId, law]));
  unique([...laws.keys()], "semantic safety law catalog");
  const cases = array(root.cases, "semantic safety spec.cases").map(
    (item, index) => parseCase(item, index, laws),
  );
  unique(cases.map((item) => item.id), "semantic safety cases");
  assertSemanticSafetyCoverage(cases);
  return {
    cases,
    id: text(root.id, "semantic safety spec.id"),
    schemaVersion: 1,
  };
}

export function planSemanticSafetySuite(
  spec: SemanticSafetySpec,
): readonly PlannedSemanticSafetyCase[] {
  const planned = spec.cases.flatMap((item) =>
    (["identity", ...item.transforms] as const).flatMap((transform) =>
      item.probes.map((probe) => {
        const snapshot = item.snapshots.find(
          (candidate) => candidate.id === probe.snapshotId,
        )!;
        return {
          contract: item.contract,
          documents: transformDocuments(snapshot.documents, transform),
          expected: probe.expected,
          id: plannedId(item.id, probe.id, transform),
          ...(probe.navigationCursor
            ? { navigationCursor: probe.navigationCursor }
            : {}),
          pairId: item.pairId,
          probeId: probe.id,
          semanticCursor: probe.semanticCursor,
          snapshotId: probe.snapshotId,
          sourceCaseId: item.id,
          transform,
        };
      }),
    ),
  );
  unique(planned.map((item) => item.id), "semantic safety plan");
  return planned;
}

export function observeSemanticSafetyCase(
  item: PlannedSemanticSafetyCase,
  results: SemanticSafetySurfaceResults,
): SemanticSafetyObservation {
  if (results.semanticView.value.kind !== "semanticView") {
    throw new Error(`${item.id}: semantic view result is missing`);
  }
  if (results.diagnostics.value.kind !== "diagnostics") {
    throw new Error(`${item.id}: diagnostics result is missing`);
  }
  const view = results.semanticView.value.view;
  const proofEvidence = view.decision.reasons
    .filter(
      (reason) => reason.kind === "proof" || reason.kind === "source-conflict",
    )
    .flatMap((reason) => reason.evidence);
  const navigationFile = item.navigationCursor?.fileId;
  const navigationPath = item.documents.find(
    (document) => document.fileId === navigationFile,
  )?.path;
  const definitions = locations(results.definition, "definition", item.id);
  const references = locations(results.references, "references", item.id);
  const preparation = results.prepareRename?.value;
  if (preparation && preparation.kind !== "renamePreparation") {
    throw new Error(`${item.id}: prepareRename result has the wrong kind`);
  }
  const editValue = results.rename?.value;
  if (editValue && editValue.kind !== "editProposal") {
    throw new Error(`${item.id}: rename result has the wrong kind`);
  }
  return {
    caseId: item.id,
    decision: view.decision.status,
    definitions,
    prepareRename: {
      ...(navigationFile ? { fileId: navigationFile } : {}),
      ...(navigationPath ? { path: navigationPath } : {}),
      ...(preparation?.kind === "renamePreparation" && preparation.placeholder
        ? { placeholder: preparation.placeholder }
        : {}),
      ...(preparation?.kind === "renamePreparation" && preparation.range
        ? { range: preparation.range }
        : {}),
    },
    problemCodes: results.diagnostics.value.diagnostics
      .filter(
        (diagnostic) =>
          diagnostic.severity === "error" || diagnostic.severity === "warning",
      )
      .map((diagnostic) => diagnostic.code)
      .sort(),
    meaningRelationId:
      view.decision.status === "established" ||
      view.decision.status === "partial"
        ? view.decision.meaning.relationId === null
          ? null
          : relationLeaf(view.decision.meaning.relationId)
        : null,
    proofGrounded:
      proofEvidence.length > 0 &&
      proofEvidence.every(
        (evidence) => evidence.sourceRanges.length > 0,
      ),
    references,
    relations: view.context.relations.map((relation) => ({
      relationId: relationLeaf(relation.relationId),
      roles: relation.roles.map((role) => ({
        role: role.role,
        symbol: role.symbol,
        ...(role.conceptId ? { conceptId: role.conceptId } : {}),
      })),
      sourceGrounded:
        relation.evidence.length > 0 &&
        relation.evidence.every(
          (evidence) => evidence.sourceRanges.length > 0,
        ),
    })),
    rename: {
      edits:
        editValue?.kind === "editProposal"
          ? (editValue.proposal?.files ?? []).flatMap((file) =>
              file.edits.map((edit) => ({
                expectedText: edit.expectedText,
                fileId: stripPlanPrefix(item, file.fileId),
                path: stripPlanPrefix(item, file.path),
                range: edit.range,
                replacementText: edit.replacementText,
              })),
            )
          : [],
      ...(editValue?.kind === "editProposal" && editValue.proposal
        ? { safety: editValue.proposal.safety }
        : {}),
    },
  };
}

export function scoreSemanticSafetySuite(
  spec: SemanticSafetySpec,
  plan: readonly PlannedSemanticSafetyCase[],
  observations: readonly SemanticSafetyObservation[],
): SemanticSafetyScorecard {
  const contractFailures: string[] = [];
  const safetyFailures: string[] = [];
  const failed = new Set<string>();
  const expectedIds = new Set(plan.map((item) => item.id));
  const byId = new Map<string, SemanticSafetyObservation>();
  for (const observation of observations) {
    if (!expectedIds.has(observation.caseId)) {
      contractFailures.push(`${observation.caseId}: unexpected observation`);
      continue;
    }
    if (byId.has(observation.caseId)) {
      contractFailures.push(`${observation.caseId}: duplicate observation`);
      failed.add(observation.caseId);
      continue;
    }
    byId.set(observation.caseId, observation);
  }
  for (const item of plan) {
    const observed = byId.get(item.id);
    if (!observed) {
      contractFailures.push(`${item.id}: missing observation`);
      failed.add(item.id);
      continue;
    }
    const itemFailures = scoreCase(item, observed);
    if (itemFailures.safety.length) {
      safetyFailures.push(`${item.id}: ${itemFailures.safety.join("; ")}`);
      failed.add(item.id);
    }
    if (itemFailures.contract.length) {
      contractFailures.push(`${item.id}: ${itemFailures.contract.join("; ")}`);
      failed.add(item.id);
    }
  }
  scoreMetamorphic(plan, byId, contractFailures, failed);
  scoreTransitions(
    spec,
    plan,
    byId,
    safetyFailures,
    contractFailures,
    failed,
  );
  const failures = [...safetyFailures, ...contractFailures];
  return {
    cases: plan.length,
    contractFailures,
    failures,
    passed: plan.length - failed.size,
    safetyFailures,
  };
}

export function resolveSemanticSafetyAnchor(
  documents: readonly CorpusDocument[],
  anchor: SemanticSafetyAnchor,
): SemanticSafetyObservedLocation {
  const document = documents.find((item) => item.fileId === anchor.fileId);
  if (!document) throw new Error(`unknown anchor file ${anchor.fileId}`);
  const matches = needleOffsets(document.content, anchor.needle);
  const selected =
    anchor.occurrence === undefined ? matches[0] : matches[anchor.occurrence];
  if (
    selected === undefined ||
    (anchor.occurrence === undefined && matches.length !== 1)
  ) {
    throw new Error(
      `${anchor.fileId}: anchor must select exactly one occurrence of ${anchor.needle}`,
    );
  }
  const selectionOffset = anchor.selection?.offset ?? 0;
  const selectionLength = anchor.selection?.length ?? anchor.needle.length;
  return {
    fileId: anchor.fileId,
    path: document.path,
    range: {
      startOffset: selected + selectionOffset,
      endOffset: selected + selectionOffset + selectionLength,
    },
  };
}

export function semanticSafetyCursorOffset(
  documents: readonly CorpusDocument[],
  cursor: SemanticSafetyCursor,
): number {
  const document = documents.find((item) => item.fileId === cursor.fileId);
  if (!document) throw new Error(`unknown cursor file ${cursor.fileId}`);
  const matches = needleOffsets(document.content, cursor.needle);
  const selected =
    cursor.occurrence === undefined ? matches[0] : matches[cursor.occurrence];
  if (
    selected === undefined ||
    (cursor.occurrence === undefined && matches.length !== 1)
  ) {
    throw new Error(
      `${cursor.fileId}: cursor must select exactly one occurrence of ${cursor.needle}`,
    );
  }
  if (cursor.offset !== undefined) return selected + cursor.offset;
  return cursor.edge === "after"
    ? selected + cursor.needle.length
    : selected;
}

export function semanticSafetyTransformApplicable(
  snapshot: SemanticSafetySnapshot,
  transform: SemanticSafetyTransform,
): boolean {
  if (transform !== "document-order") return true;
  if (snapshot.documents.length < 2) return false;
  const includeDirective =
    /\\(?:include|input|subfile|import|includefrom|subimport)\s*(?:\[[^\]]*\]\s*)?\{/u;
  return snapshot.documents.every(
    (document) => !includeDirective.test(document.content),
  );
}

function parseCase(
  value: unknown,
  index: number,
  laws: ReadonlyMap<string, SemanticSafetyLawCatalogEntry>,
): SemanticSafetyCase {
  const path = `semantic safety spec.cases[${index}]`;
  const item = record(value, path);
  exact(
    item,
    [
      "id",
      "pairId",
      "contract",
      "lawIds",
      "snapshots",
      "probes",
      "transforms",
      "transitions",
      "variationTags",
    ],
    path,
  );
  const snapshots = array(item.snapshots, `${path}.snapshots`).map(
    (snapshot, snapshotIndex) =>
      parseSnapshot(snapshot, `${path}.snapshots[${snapshotIndex}]`),
  );
  if (!snapshots.length) throw new Error(`${path}.snapshots: must not be empty`);
  unique(snapshots.map((snapshot) => snapshot.id), `${path}.snapshots`);
  const lawIds = strings(item.lawIds, `${path}.lawIds`);
  for (const lawId of lawIds) {
    if (!laws.has(lawId)) throw new Error(`${path}: unknown law ${lawId}`);
  }
  const probes = array(item.probes, `${path}.probes`).map((probe, probeIndex) =>
    parseProbe(
      probe,
      `${path}.probes[${probeIndex}]`,
      snapshots,
      lawIds,
      laws,
    ),
  );
  if (!probes.length) throw new Error(`${path}.probes: must not be empty`);
  unique(probes.map((probe) => probe.id), `${path}.probes`);
  const transforms = array(item.transforms, `${path}.transforms`).map(
    (transform, transformIndex) =>
      oneOf(
        transform,
        SEMANTIC_SAFETY_TRANSFORMS,
        `${path}.transforms[${transformIndex}]`,
      ),
  );
  unique(transforms, `${path}.transforms`);
  if (
    transforms.includes("document-order") &&
    snapshots.some(
      (snapshot) =>
        !semanticSafetyTransformApplicable(snapshot, "document-order"),
    )
  ) {
    throw new Error(
      `${path}: document-order requires multiple disconnected documents without include/import source-order semantics`,
    );
  }
  if (
    transforms.includes("opposition-order") &&
    snapshots.some((snapshot) =>
      snapshot.documents.some(
        (document) => document.content.split("\n\n").length !== 2,
      ),
    )
  ) {
    throw new Error(`${path}: opposition-order requires two paragraphs per document`);
  }
  const transitions = array(item.transitions, `${path}.transitions`).map(
    (transition, transitionIndex) =>
      parseTransition(
        transition,
        `${path}.transitions[${transitionIndex}]`,
        probes,
        snapshots,
      ),
  );
  return {
    contract: oneOf(
      item.contract,
      SEMANTIC_SAFETY_CONTRACTS,
      `${path}.contract`,
    ),
    id: text(item.id, `${path}.id`),
    lawIds,
    pairId: text(item.pairId, `${path}.pairId`),
    probes,
    snapshots,
    transforms,
    transitions,
    variationTags: strings(item.variationTags, `${path}.variationTags`, 2),
  };
}

function parseSnapshot(value: unknown, path: string): SemanticSafetySnapshot {
  const item = record(value, path);
  exact(item, ["id", "documents"], path);
  const documents = array(item.documents, `${path}.documents`).map(
    (document, documentIndex) => {
      const documentPath = `${path}.documents[${documentIndex}]`;
      const entry = record(document, documentPath);
      exact(entry, ["fileId", "path", "content"], documentPath);
      return {
        content: text(entry.content, `${documentPath}.content`),
        fileId: text(entry.fileId, `${documentPath}.fileId`),
        path: text(entry.path, `${documentPath}.path`),
      };
    },
  );
  if (!documents.length) throw new Error(`${path}.documents: must not be empty`);
  unique(documents.map((document) => document.fileId), `${path}.documents.fileId`);
  unique(documents.map((document) => document.path), `${path}.documents.path`);
  return { documents, id: text(item.id, `${path}.id`) };
}

function parseProbe(
  value: unknown,
  path: string,
  snapshots: readonly SemanticSafetySnapshot[],
  lawIds: readonly string[],
  laws: ReadonlyMap<string, SemanticSafetyLawCatalogEntry>,
): SemanticSafetyProbe {
  const item = record(value, path);
  exact(
    item,
    ["id", "snapshotId", "semanticCursor", "navigationCursor", "expected"],
    path,
    ["navigationCursor"],
  );
  const snapshotId = text(item.snapshotId, `${path}.snapshotId`);
  const snapshot = snapshots.find((candidate) => candidate.id === snapshotId);
  if (!snapshot) throw new Error(`${path}: unknown snapshot ${snapshotId}`);
  const semanticCursor = parseCursor(
    item.semanticCursor,
    `${path}.semanticCursor`,
  );
  semanticSafetyCursorOffset(snapshot.documents, semanticCursor);
  const navigationCursor =
    item.navigationCursor === undefined
      ? undefined
      : parseCursor(item.navigationCursor, `${path}.navigationCursor`);
  if (navigationCursor) {
    semanticSafetyCursorOffset(snapshot.documents, navigationCursor);
  }
  const expected = parseExpectation(
    item.expected,
    `${path}.expected`,
    snapshot.documents,
    lawIds,
    laws,
  );
  if (expected.navigation.mode !== "skip" && !navigationCursor) {
    throw new Error(`${path}: navigation expectation requires a navigation cursor`);
  }
  if (expected.navigation.mode === "skip" && navigationCursor) {
    throw new Error(`${path}: navigation cursor requires an exact or reject expectation`);
  }
  return {
    expected,
    id: text(item.id, `${path}.id`),
    ...(navigationCursor ? { navigationCursor } : {}),
    semanticCursor,
    snapshotId,
  };
}

function parseCursor(value: unknown, path: string): SemanticSafetyCursor {
  const item = record(value, path);
  exact(
    item,
    ["fileId", "needle", "occurrence", "edge", "offset"],
    path,
    ["occurrence", "edge", "offset"],
  );
  if ((item.edge === undefined) === (item.offset === undefined)) {
    throw new Error(`${path}: exactly one of edge or offset is required`);
  }
  const needle = text(item.needle, `${path}.needle`);
  const offset =
    item.offset === undefined ? undefined : integer(item.offset, `${path}.offset`);
  if (offset !== undefined && offset > needle.length) {
    throw new Error(`${path}.offset: outside cursor needle`);
  }
  return {
    ...(item.edge === undefined
      ? {}
      : { edge: oneOf(item.edge, ["after", "before"] as const, `${path}.edge`) }),
    fileId: text(item.fileId, `${path}.fileId`),
    needle,
    ...(item.occurrence === undefined
      ? {}
      : { occurrence: integer(item.occurrence, `${path}.occurrence`) }),
    ...(offset === undefined ? {} : { offset }),
  };
}

function parseExpectation(
  value: unknown,
  path: string,
  documents: readonly CorpusDocument[],
  lawIds: readonly string[],
  laws: ReadonlyMap<string, SemanticSafetyLawCatalogEntry>,
): SemanticSafetyExpectation {
  const item = record(value, path);
  exact(
    item,
    [
      "decision",
      "proofGrounded",
      "relations",
      "excludedRelationIds",
      "maximumProblems",
      "navigation",
    ],
    path,
  );
  const relations = array(item.relations, `${path}.relations`).map(
    (relation, relationIndex) => {
      const relationPath = `${path}.relations[${relationIndex}]`;
      const entry = record(relation, relationPath);
      exact(entry, ["relationId", "roles", "sourceGrounded"], relationPath);
      const relationId = text(entry.relationId, `${relationPath}.relationId`);
      const law = laws.get(relationId);
      if (!law) throw new Error(`${relationPath}: unknown law ${relationId}`);
      if (!lawIds.includes(relationId)) {
        throw new Error(`${relationPath}: law is absent from case lawIds`);
      }
      const roles = array(entry.roles, `${relationPath}.roles`).map(
        (role, roleIndex) => {
          const rolePath = `${relationPath}.roles[${roleIndex}]`;
          const roleEntry = record(role, rolePath);
          exact(roleEntry, ["role", "symbol"], rolePath);
          return {
            role: text(roleEntry.role, `${rolePath}.role`),
            symbol: text(roleEntry.symbol, `${rolePath}.symbol`),
          };
        },
      );
      const actualRoles = [...roles.map((role) => role.role)].sort();
      const expectedRoles = [...law.roles].sort();
      if (
        actualRoles.length !== expectedRoles.length ||
        actualRoles.some((role, index) => role !== expectedRoles[index])
      ) {
        throw new Error(`${relationPath}: roles do not match ${relationId}`);
      }
      return {
        relationId,
        roles,
        sourceGrounded: boolean(
          entry.sourceGrounded,
          `${relationPath}.sourceGrounded`,
        ),
      };
    },
  );
  const excludedRelationIds = strings(
    item.excludedRelationIds,
    `${path}.excludedRelationIds`,
  );
  for (const relationId of excludedRelationIds) {
    if (!laws.has(relationId)) {
      throw new Error(`${path}: unknown excluded law ${relationId}`);
    }
  }
  return {
    decision: oneOf(
      item.decision,
      [
        "ambiguous",
        "conflicting",
        "established",
        "partial",
        "unsupported",
      ] as const,
      `${path}.decision`,
    ),
    excludedRelationIds,
    maximumProblems: integer(item.maximumProblems, `${path}.maximumProblems`),
    navigation: parseNavigation(item.navigation, `${path}.navigation`, documents),
    proofGrounded: boolean(item.proofGrounded, `${path}.proofGrounded`),
    relations,
  };
}

function parseNavigation(
  value: unknown,
  path: string,
  documents: readonly CorpusDocument[],
): SemanticSafetyNavigationExpectation {
  const item = record(value, path);
  const mode = oneOf(item.mode, ["skip", "reject", "exact"] as const, `${path}.mode`);
  if (mode !== "exact") {
    exact(item, ["mode"], path);
    return { mode };
  }
  exact(
    item,
    [
      "mode",
      "definition",
      "references",
      "rename",
      "placeholder",
      "newName",
      "expectedText",
      "replacementText",
      "safety",
    ],
    path,
  );
  const anchors = (key: "definition" | "references" | "rename") =>
    array(item[key], `${path}.${key}`).map((anchor, index) => {
      const parsed = parseAnchor(anchor, `${path}.${key}[${index}]`);
      resolveSemanticSafetyAnchor(documents, parsed);
      return parsed;
    });
  const definition = anchors("definition");
  const references = anchors("references");
  const rename = anchors("rename");
  if (!definition.length || !references.length || !rename.length) {
    throw new Error(`${path}: exact navigation requires complete non-empty surfaces`);
  }
  return {
    definition,
    expectedText: text(item.expectedText, `${path}.expectedText`),
    mode,
    newName: text(item.newName, `${path}.newName`),
    placeholder: text(item.placeholder, `${path}.placeholder`),
    references,
    rename,
    replacementText: text(item.replacementText, `${path}.replacementText`),
    safety: oneOf(
      item.safety,
      ["deterministic", "review-required"] as const,
      `${path}.safety`,
    ),
  };
}

function parseAnchor(value: unknown, path: string): SemanticSafetyAnchor {
  const item = record(value, path);
  exact(
    item,
    ["fileId", "needle", "occurrence", "selection"],
    path,
    ["occurrence", "selection"],
  );
  const needle = text(item.needle, `${path}.needle`);
  const selection =
    item.selection === undefined
      ? undefined
      : (() => {
          const entry = record(item.selection, `${path}.selection`);
          exact(entry, ["offset", "length"], `${path}.selection`);
          const offset = integer(entry.offset, `${path}.selection.offset`);
          const length = positiveInteger(
            entry.length,
            `${path}.selection.length`,
          );
          if (offset + length > needle.length) {
            throw new Error(`${path}.selection: outside anchor needle`);
          }
          return { length, offset };
        })();
  return {
    fileId: text(item.fileId, `${path}.fileId`),
    needle,
    ...(item.occurrence === undefined
      ? {}
      : { occurrence: integer(item.occurrence, `${path}.occurrence`) }),
    ...(selection ? { selection } : {}),
  };
}

function parseTransition(
  value: unknown,
  path: string,
  probes: readonly SemanticSafetyProbe[],
  snapshots: readonly SemanticSafetySnapshot[],
): SemanticSafetyTransition {
  const item = record(value, path);
  exact(item, ["kind", "fromProbeId", "toProbeId", "relationId"], path);
  const fromProbeId = text(item.fromProbeId, `${path}.fromProbeId`);
  const toProbeId = text(item.toProbeId, `${path}.toProbeId`);
  const fromIndex = probes.findIndex((probe) => probe.id === fromProbeId);
  const toIndex = probes.findIndex((probe) => probe.id === toProbeId);
  if (fromIndex < 0 || toIndex < 0) throw new Error(`${path}: unknown probe`);
  const fromSnapshot = snapshots.findIndex(
    (snapshot) => snapshot.id === probes[fromIndex]!.snapshotId,
  );
  const toSnapshot = snapshots.findIndex(
    (snapshot) => snapshot.id === probes[toIndex]!.snapshotId,
  );
  if (fromSnapshot >= toSnapshot) {
    throw new Error(`${path}: lifecycle transition must move forward`);
  }
  return {
    fromProbeId,
    kind: oneOf(
      item.kind,
      ["retract-relation"] as const,
      `${path}.kind`,
    ),
    relationId: text(item.relationId, `${path}.relationId`),
    toProbeId,
  };
}

function assertSemanticSafetyCoverage(
  cases: readonly SemanticSafetyCase[],
): void {
  for (const contract of SEMANTIC_SAFETY_CONTRACTS) {
    if (!cases.some((item) => item.contract === contract)) {
      throw new Error(`semantic safety spec: missing ${contract}`);
    }
  }
  const probesByPair = new Map<string, SemanticSafetyProbe[]>();
  for (const item of cases) {
    const probes = probesByPair.get(item.pairId) ?? [];
    probes.push(...item.probes);
    probesByPair.set(item.pairId, probes);
    if (item.contract === "lifecycle-retraction" && !item.transitions.length) {
      throw new Error(`${item.id}: lifecycle contract requires a transition`);
    }
    if (
      item.contract === "navigation-complete" &&
      !item.probes.some((probe) => probe.expected.navigation.mode === "exact")
    ) {
      throw new Error(`${item.id}: navigation-complete requires exact navigation`);
    }
    if (
      item.contract === "navigation-reject" &&
      !item.probes.some((probe) => probe.expected.navigation.mode === "reject")
    ) {
      throw new Error(`${item.id}: navigation-reject requires rejection`);
    }
  }
  for (const [pairId, probes] of probesByPair) {
    if (probes.length < 2) throw new Error(`${pairId}: minimal pair is incomplete`);
    if (new Set(probes.map((probe) => probe.expected.decision)).size < 2) {
      throw new Error(`${pairId}: minimal pair must change the decision`);
    }
  }
}

function transformDocuments(
  documents: readonly CorpusDocument[],
  transform: "identity" | SemanticSafetyTransform,
): readonly CorpusDocument[] {
  const cloned = documents.map((document) => ({ ...document }));
  if (transform === "identity") return cloned;
  if (transform === "document-order") return cloned.reverse();
  return cloned.map((document) => {
    if (transform === "neutral-prefix") {
      return {
        ...document,
        content: `Editorial note: notation and claims below remain unchanged.\n${document.content}`,
      };
    }
    if (transform === "trailing-comment") {
      return {
        ...document,
        content: `${document.content}\n% v030 semantic-safety metamorphic invariant`,
      };
    }
    return {
      ...document,
      content: document.content.split("\n\n").reverse().join("\n\n"),
    };
  });
}

function scoreCase(
  item: PlannedSemanticSafetyCase,
  observed: SemanticSafetyObservation,
): { readonly contract: string[]; readonly safety: string[] } {
  const contract: string[] = [];
  const safety: string[] = [];
  if (observed.decision !== item.expected.decision) {
    const message = `decision ${observed.decision}; expected ${item.expected.decision}`;
    if (
      (observed.decision === "established" &&
        item.expected.decision !== "established") ||
      (observed.decision === "conflicting" &&
        item.expected.decision !== "conflicting")
    ) {
      safety.push(message);
    } else {
      contract.push(message);
    }
  }
  if (observed.proofGrounded !== item.expected.proofGrounded) {
    const message = `proof grounding ${observed.proofGrounded}; expected ${item.expected.proofGrounded}`;
    (observed.proofGrounded ? safety : contract).push(message);
  }
  for (const expected of item.expected.relations) {
    const relation = observed.relations.find(
      (candidate) =>
        relationLeaf(candidate.relationId) === expected.relationId &&
        roleInstancesMatch(candidate.roles, expected.roles, undefined),
    );
    if (!relation) {
      contract.push(`missing relation ${expected.relationId}`);
    } else if (relation.sourceGrounded !== expected.sourceGrounded) {
      const message = `${expected.relationId} source grounding ${relation.sourceGrounded}; expected ${expected.sourceGrounded}`;
      (relation.sourceGrounded ? safety : contract).push(message);
    }
  }
  for (const excluded of item.expected.excludedRelationIds) {
    if (
      observed.meaningRelationId === excluded ||
      observed.relations.some(
        (relation) => relationLeaf(relation.relationId) === excluded,
      )
    ) {
      safety.push(`leaked relation ${excluded}`);
    }
  }
  if (observed.problemCodes.length > item.expected.maximumProblems) {
    safety.push(
      `problems ${observed.problemCodes.length}; expected at most ${item.expected.maximumProblems}`,
    );
  }
  safety.push(...scoreNavigation(item, observed));
  return { contract, safety };
}

function scoreNavigation(
  item: PlannedSemanticSafetyCase,
  observed: SemanticSafetyObservation,
): string[] {
  const expected = item.expected.navigation;
  if (expected.mode === "skip") return [];
  if (expected.mode === "reject") {
    const failures: string[] = [];
    if (observed.definitions.length) failures.push("definition was not rejected");
    if (observed.references.length) failures.push("references were not rejected");
    if (observed.prepareRename.range) failures.push("prepareRename was not rejected");
    if (observed.rename.edits.length || observed.rename.safety) {
      failures.push("rename was not rejected atomically");
    }
    return failures;
  }
  const failures: string[] = [];
  const expectedDefinitions = expected.definition.map((anchor) =>
    resolveSemanticSafetyAnchor(item.documents, anchor),
  );
  const expectedReferences = expected.references.map((anchor) =>
    resolveSemanticSafetyAnchor(item.documents, anchor),
  );
  const expectedRename = expected.rename.map((anchor) =>
    resolveSemanticSafetyAnchor(item.documents, anchor),
  );
  if (!sameLocations(observed.definitions, expectedDefinitions)) {
    failures.push("definition locations are incomplete or contain extras");
  }
  if (!sameLocations(observed.references, expectedReferences)) {
    failures.push("reference locations are incomplete or contain extras");
  }
  const cursor = item.navigationCursor!;
  const cursorDocument = item.documents.find(
    (document) => document.fileId === cursor.fileId,
  )!;
  const cursorOffset = semanticSafetyCursorOffset(item.documents, cursor);
  const expectedPreparation = {
    fileId: cursor.fileId,
    path: cursorDocument.path,
    range: symbolRangeAt(cursorDocument.content, cursorOffset, expected.expectedText),
  };
  if (
    observed.prepareRename.placeholder !== expected.placeholder ||
    observed.prepareRename.fileId !== expectedPreparation.fileId ||
    observed.prepareRename.path !== expectedPreparation.path ||
    !observed.prepareRename.range ||
    !sameRange(observed.prepareRename.range, expectedPreparation.range)
  ) {
    failures.push("prepareRename contract differs");
  }
  const actualRenameLocations = observed.rename.edits.map(
    ({ expectedText: _expectedText, replacementText: _replacementText, ...location }) =>
      location,
  );
  if (!sameLocations(actualRenameLocations, expectedRename)) {
    failures.push("rename edits are incomplete or contain extras");
  }
  if (
    observed.rename.edits.some(
      (edit) =>
        edit.expectedText !== expected.expectedText ||
        edit.replacementText !== expected.replacementText,
    )
  ) {
    failures.push("rename edit text differs");
  }
  if (observed.rename.safety !== expected.safety) {
    failures.push(`rename safety ${observed.rename.safety ?? "missing"}; expected ${expected.safety}`);
  }
  return failures;
}

function scoreMetamorphic(
  plan: readonly PlannedSemanticSafetyCase[],
  observations: ReadonlyMap<string, SemanticSafetyObservation>,
  failures: string[],
  failed: Set<string>,
): void {
  for (const item of plan) {
    if (item.transform === "identity") continue;
    const base = plan.find(
      (candidate) =>
        candidate.sourceCaseId === item.sourceCaseId &&
        candidate.probeId === item.probeId &&
        candidate.transform === "identity",
    )!;
    const baseObservation = observations.get(base.id);
    const transformedObservation = observations.get(item.id);
    if (!baseObservation || !transformedObservation) continue;
    if (
      JSON.stringify(stableObservation(baseObservation)) !==
      JSON.stringify(stableObservation(transformedObservation))
    ) {
      failures.push(`${item.id}: metamorphic result differs from ${base.id}`);
      failed.add(item.id);
    }
  }
}

function scoreTransitions(
  spec: SemanticSafetySpec,
  plan: readonly PlannedSemanticSafetyCase[],
  observations: ReadonlyMap<string, SemanticSafetyObservation>,
  safetyFailures: string[],
  contractFailures: string[],
  failed: Set<string>,
): void {
  for (const item of spec.cases) {
    for (const transition of item.transitions) {
      for (const transform of ["identity", ...item.transforms] as const) {
        const from = plan.find(
          (candidate) =>
            candidate.sourceCaseId === item.id &&
            candidate.probeId === transition.fromProbeId &&
            candidate.transform === transform,
        )!;
        const to = plan.find(
          (candidate) =>
            candidate.sourceCaseId === item.id &&
            candidate.probeId === transition.toProbeId &&
            candidate.transform === transform,
        )!;
        const before = observations.get(from.id);
        const after = observations.get(to.id);
        if (!before || !after) continue;
        const establishedBefore =
          before.decision === "established" && before.proofGrounded;
        const presentAfter = after.relations.some(
          (relation) => relationLeaf(relation.relationId) === transition.relationId,
        ) || after.meaningRelationId === transition.relationId;
        if (!establishedBefore) {
          contractFailures.push(
            `${from.id}: transition source did not establish ${transition.relationId}`,
          );
          failed.add(from.id);
        }
        if (presentAfter) {
          safetyFailures.push(
            `${to.id}: retained ${transition.relationId} after retraction from ${from.id}`,
          );
          failed.add(to.id);
        }
      }
    }
  }
}

function stableObservation(observation: SemanticSafetyObservation): unknown {
  return {
    decision: observation.decision,
    definitionCount: observation.definitions.length,
    prepareRename: Boolean(observation.prepareRename.range),
    problemCodes: [...observation.problemCodes].sort(),
    meaningRelationId: observation.meaningRelationId,
    proofGrounded: observation.proofGrounded,
    referenceCount: observation.references.length,
    relations: observation.relations
      .map((relation) => ({
        relationId: relationLeaf(relation.relationId),
        roles: relation.roles
          .map((role) => `${role.role}:${normalizeSymbol(role.symbol, undefined)}`)
          .sort(),
        sourceGrounded: relation.sourceGrounded,
      }))
      .sort((left, right) => left.relationId.localeCompare(right.relationId)),
    rename: {
      edits: observation.rename.edits
        .map((edit) => `${edit.expectedText}->${edit.replacementText}`)
        .sort(),
      safety: observation.rename.safety ?? null,
    },
  };
}

function locations(
  result: QueryResult | undefined,
  surface: string,
  caseId: string,
): SemanticSafetyObservedLocation[] {
  if (!result) return [];
  if (result.value.kind !== "locations") {
    throw new Error(`${caseId}: ${surface} result has the wrong kind`);
  }
  const prefix = `${caseId}/`;
  return result.value.locations.map((location) => ({
    fileId: location.fileId.startsWith(prefix)
      ? location.fileId.slice(prefix.length)
      : location.fileId,
    path: location.path.startsWith(prefix)
      ? location.path.slice(prefix.length)
      : location.path,
    range: location.range,
  }));
}

function stripPlanPrefix(item: PlannedSemanticSafetyCase, value: string): string {
  const prefix = `${item.id}/`;
  return value.startsWith(prefix) ? value.slice(prefix.length) : value;
}

function sameLocations(
  actual: readonly SemanticSafetyObservedLocation[],
  expected: readonly SemanticSafetyObservedLocation[],
): boolean {
  const keys = (locations: readonly SemanticSafetyObservedLocation[]) =>
    locations
      .map(
        (location) =>
          `${location.fileId}\u0000${location.path}\u0000${location.range.startOffset}\u0000${location.range.endOffset}`,
      )
      .sort();
  return JSON.stringify(keys(actual)) === JSON.stringify(keys(expected));
}

function symbolRangeAt(
  content: string,
  cursorOffset: number,
  expectedText: string,
): SourceRange {
  const candidates = needleOffsets(content, expectedText).filter(
    (offset) => offset <= cursorOffset && cursorOffset <= offset + expectedText.length,
  );
  if (candidates.length !== 1) {
    throw new Error(`navigation cursor does not identify ${expectedText}`);
  }
  return {
    startOffset: candidates[0]!,
    endOffset: candidates[0]! + expectedText.length,
  };
}

function plannedId(
  caseId: string,
  probeId: string,
  transform: "identity" | SemanticSafetyTransform,
): string {
  return `${caseId}/${probeId}@${transform}`;
}

function relationLeaf(relationId: string): string {
  return relationId.slice(relationId.lastIndexOf(":") + 1);
}

function needleOffsets(content: string, needle: string): number[] {
  const offsets: number[] = [];
  for (
    let offset = content.indexOf(needle);
    offset >= 0;
    offset = content.indexOf(needle, offset + Math.max(needle.length, 1))
  ) {
    offsets.push(offset);
  }
  return offsets;
}

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return left.startOffset === right.startOffset && left.endOffset === right.endOffset;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path}: must be an array`);
  return value;
}

function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  path: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown) throw new Error(`${path}.${unknown}: unknown field`);
  const missing = keys.find(
    (key) => !optional.includes(key) && !Object.hasOwn(value, key),
  );
  if (missing) throw new Error(`${path}.${missing}: missing field`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path}: must be non-empty text`);
  }
  return value;
}

function strings(value: unknown, path: string, minimum = 0): string[] {
  const result = array(value, path).map((item, index) =>
    text(item, `${path}[${index}]`),
  );
  if (result.length < minimum) {
    throw new Error(`${path}: requires at least ${minimum} values`);
  }
  unique(result, path);
  return result;
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`${path}: must be a non-negative integer`);
  }
  return value as number;
}

function positiveInteger(value: unknown, path: string): number {
  const result = integer(value, path);
  if (result === 0) throw new Error(`${path}: must be positive`);
  return result;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path}: must be boolean`);
  return value;
}

function oneOf<const T extends string>(
  value: unknown,
  values: readonly T[],
  path: string,
): T {
  const result = text(value, path);
  if (!values.includes(result as T)) {
    throw new Error(`${path}: invalid value ${result}`);
  }
  return result as T;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) {
    throw new Error(`${path}: values must be unique`);
  }
}
