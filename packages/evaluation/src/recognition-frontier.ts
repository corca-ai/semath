import type { SemanticViewInfo } from "../../../packages/protocol/src/index";
import type { CorpusDocument } from "./model";

export const RECOGNITION_FRONTIER_STAGES = [
  "syntax-unavailable",
  "canonical-unsupported",
  "discourse-evidence-missing",
  "identity-scope-unresolved",
  "structural-candidate-missing",
  "type-condition-evidence-missing",
  "genuine-ambiguity",
  "demonstrated-conflict",
  "established",
] as const;

export type RecognitionFrontierStage =
  (typeof RECOGNITION_FRONTIER_STAGES)[number];
export type RecognitionDecision = SemanticViewInfo["decision"]["status"];

export interface RecognitionFrontierSignals {
  readonly canonicalAvailable: boolean;
  readonly decision: RecognitionDecision;
  readonly discourseEvidence: boolean;
  readonly engineLimited: boolean;
  readonly identityResolved: boolean;
  readonly sourceGroundedConflict: boolean;
  readonly structuralCandidates: boolean;
  readonly syntaxAvailable: boolean;
  readonly typeOrConditionEvidence: boolean;
}

export interface RecognitionFrontierCase {
  readonly baseline: {
    readonly decision: RecognitionDecision;
    readonly stage: RecognitionFrontierStage;
  };
  readonly cursor: { readonly fileId: string; readonly needle: string };
  readonly documents: readonly CorpusDocument[];
  readonly family: string;
  readonly id: string;
  readonly target: {
    readonly decision: RecognitionDecision;
    readonly relationId: string | null;
    readonly stage: RecognitionFrontierStage;
  };
  readonly variationTags: readonly string[];
}

export interface RecognitionFrontier {
  readonly baseline: {
    readonly commit: string;
    readonly note: string;
    readonly protocolVersion: number;
  };
  readonly cases: readonly RecognitionFrontierCase[];
  readonly schemaVersion: 1;
}

export interface RecognitionFrontierObservation {
  readonly caseId: string;
  readonly decision: RecognitionDecision;
  readonly relationId: string | null;
  readonly signals: RecognitionFrontierSignals;
  readonly stage: RecognitionFrontierStage;
}

export interface RecognitionFrontierScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly firstFailure?: string;
  readonly passed: number;
  readonly risk: {
    readonly falseConflict: number;
    readonly falseEstablishment: number;
    readonly missedCoverage: number;
    readonly total: number;
  };
  readonly stages: Readonly<
    Record<RecognitionFrontierStage, { readonly passed: number; readonly total: number }>
  >;
}

export function classifyRecognitionFrontier(
  signals: RecognitionFrontierSignals,
): RecognitionFrontierStage {
  if (!signals.syntaxAvailable) return "syntax-unavailable";
  if (signals.decision === "conflicting" && signals.sourceGroundedConflict) {
    return "demonstrated-conflict";
  }
  if (signals.decision === "ambiguous") return "genuine-ambiguity";
  if (signals.decision === "established") return "established";
  if (!signals.canonicalAvailable || signals.engineLimited) {
    return "canonical-unsupported";
  }
  if (!signals.discourseEvidence) return "discourse-evidence-missing";
  if (!signals.identityResolved) return "identity-scope-unresolved";
  if (!signals.structuralCandidates) return "structural-candidate-missing";
  return "type-condition-evidence-missing";
}

export function frontierSignals(
  view: SemanticViewInfo,
  syntaxAvailable: boolean,
): RecognitionFrontierSignals {
  const reasons = view.decision.reasons;
  const requirements =
    view.decision.status === "partial" ? view.decision.requirements : [];
  const sourceGroundedConflict =
    view.decision.status === "conflicting" &&
    view.decision.conflicts.some((conflict) =>
      conflict.evidence.some((evidence) => evidence.sourceRanges.length > 0),
    );
  return {
    canonicalAvailable:
      Boolean(view.symbol) ||
      view.context.candidates.length > 0 ||
      view.context.relations.length > 0,
    decision: view.decision.status,
    discourseEvidence:
      view.context.claims.length > 0 ||
      view.context.concepts.length > 0 ||
      view.context.quantities.length > 0 ||
      Boolean(
        view.symbol?.definitions.length ||
          view.symbol?.roles?.length ||
          view.symbol?.shapes.length,
      ) ||
      view.domains.some((domain) =>
        domain.evidence.some((evidence) =>
          ["domain-context", "prose-domain-prior"].includes(evidence.kind),
        ),
      ),
    engineLimited: reasons.some((reason) => reason.kind === "engine-limit"),
    identityResolved:
      Boolean(
        view.context.entityId ||
          view.symbol?.entityId ||
          view.symbol?.definitions.length,
      ) ||
      ((view.decision.status === "established" ||
        view.decision.status === "partial") &&
        view.decision.meaning.relationId !== null),
    sourceGroundedConflict,
    structuralCandidates:
      view.context.candidates.length > 0 || view.context.relations.length > 0,
    syntaxAvailable,
    typeOrConditionEvidence:
      requirements.some((requirement) => requirement.evidence.length > 0) ||
      view.context.quantities.length > 0 ||
      Boolean(view.symbol?.roles?.length || view.symbol?.shapes.length),
  };
}

export function parseRecognitionFrontier(value: unknown): RecognitionFrontier {
  const root = record(value, "recognition frontier");
  exact(root, ["schemaVersion", "baseline", "cases"], "recognition frontier");
  if (root.schemaVersion !== 1) {
    throw new Error("recognition frontier.schemaVersion: must be 1");
  }
  const baseline = record(root.baseline, "recognition frontier.baseline");
  exact(
    baseline,
    ["commit", "protocolVersion", "note"],
    "recognition frontier.baseline",
  );
  if (!Array.isArray(root.cases) || root.cases.length < 24) {
    throw new Error(
      "recognition frontier.cases: must contain at least 24 independently authored cases",
    );
  }
  const cases = root.cases.map(parseCase);
  unique(cases.map((item) => item.id), "recognition frontier.cases.id");
  const families = new Map<string, number>();
  for (const item of cases) {
    families.set(item.family, (families.get(item.family) ?? 0) + 1);
  }
  if (families.size < 6 || [...families.values()].some((count) => count < 4)) {
    throw new Error(
      "recognition frontier.cases: requires at least 6 families with 4 cases each",
    );
  }
  return {
    baseline: {
      commit: text(baseline.commit, "recognition frontier.baseline.commit"),
      note: text(baseline.note, "recognition frontier.baseline.note"),
      protocolVersion: integer(
        baseline.protocolVersion,
        "recognition frontier.baseline.protocolVersion",
      ),
    },
    cases,
    schemaVersion: 1,
  };
}

export function scoreRecognitionFrontier(
  frontier: RecognitionFrontier,
  observations: readonly RecognitionFrontierObservation[],
): RecognitionFrontierScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  const stages = Object.fromEntries(
    RECOGNITION_FRONTIER_STAGES.map((stage) => [
      stage,
      { passed: 0, total: 0 },
    ]),
  ) as Record<
    RecognitionFrontierStage,
    { passed: number; total: number }
  >;
  let falseConflict = 0;
  let falseEstablishment = 0;
  let missedCoverage = 0;
  if (byId.size !== observations.length) failures.push("duplicate observations");
  for (const item of frontier.cases) {
    const expected = item.target;
    stages[expected.stage].total += 1;
    const observed = byId.get(item.id);
    if (!observed) {
      failures.push(`${item.id}: missing observation`);
      missedCoverage += 1;
      continue;
    }
    const caseFailures: string[] = [];
    if (observed.decision !== expected.decision) {
      caseFailures.push(
        `decision ${observed.decision}; expected ${expected.decision}`,
      );
      if (
        observed.decision === "established" &&
        expected.decision !== "established"
      ) {
        falseEstablishment += 1;
      } else if (
        observed.decision === "conflicting" &&
        expected.decision !== "conflicting"
      ) {
        falseConflict += 1;
      } else {
        missedCoverage += 1;
      }
    }
    if (observed.stage !== expected.stage) {
      caseFailures.push(`stage ${observed.stage}; expected ${expected.stage}`);
    }
    if (
      expected.relationId !== null &&
      observed.relationId !== expected.relationId
    ) {
      caseFailures.push(
        `relation ${observed.relationId}; expected ${expected.relationId}`,
      );
      if (observed.decision === "established") falseEstablishment += 1;
      else missedCoverage += 1;
    }
    if (caseFailures.length) {
      failures.push(`${item.id}: ${caseFailures.join("; ")}`);
    } else {
      stages[expected.stage].passed += 1;
    }
  }
  return {
    cases: frontier.cases.length,
    failures,
    ...(failures[0] ? { firstFailure: failures[0] } : {}),
    passed:
      frontier.cases.length -
      new Set(failures.map((failure) => failure.split(":", 1)[0])).size,
    risk: {
      falseConflict,
      falseEstablishment,
      missedCoverage,
      total: falseConflict * 8 + falseEstablishment * 8 + missedCoverage * 2,
    },
    stages,
  };
}

function parseCase(value: unknown, index: number): RecognitionFrontierCase {
  const path = `recognition frontier.cases[${index}]`;
  const item = record(value, path);
  exact(
    item,
    [
      "id",
      "family",
      "documents",
      "cursor",
      "baseline",
      "target",
      "variationTags",
    ],
    path,
  );
  if (!Array.isArray(item.documents) || !item.documents.length) {
    throw new Error(`${path}.documents: must not be empty`);
  }
  const documents = item.documents.map((value, documentIndex) => {
    const documentPath = `${path}.documents[${documentIndex}]`;
    const document = record(value, documentPath);
    exact(document, ["fileId", "path", "content"], documentPath);
    return {
      content: text(document.content, `${documentPath}.content`),
      fileId: text(document.fileId, `${documentPath}.fileId`),
      path: text(document.path, `${documentPath}.path`),
    };
  });
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(cursor, ["fileId", "needle"], `${path}.cursor`);
  const baseline = parseExpectation(item.baseline, `${path}.baseline`, false);
  const target = parseExpectation(item.target, `${path}.target`, true);
  if (!Array.isArray(item.variationTags) || !item.variationTags.length) {
    throw new Error(`${path}.variationTags: must not be empty`);
  }
  return {
    baseline: { decision: baseline.decision, stage: baseline.stage },
    cursor: {
      fileId: text(cursor.fileId, `${path}.cursor.fileId`),
      needle: text(cursor.needle, `${path}.cursor.needle`),
    },
    documents,
    family: text(item.family, `${path}.family`),
    id: text(item.id, `${path}.id`),
    target: {
      decision: target.decision,
      relationId: target.relationId,
      stage: target.stage,
    },
    variationTags: item.variationTags.map((tag, tagIndex) =>
      text(tag, `${path}.variationTags[${tagIndex}]`),
    ),
  };
}

function parseExpectation(
  value: unknown,
  path: string,
  relation: boolean,
): {
  decision: RecognitionDecision;
  relationId: string | null;
  stage: RecognitionFrontierStage;
} {
  const item = record(value, path);
  exact(item, relation ? ["decision", "stage", "relationId"] : ["decision", "stage"], path);
  const decision = text(item.decision, `${path}.decision`);
  if (!isDecision(decision)) throw new Error(`${path}.decision: invalid decision`);
  const stage = text(item.stage, `${path}.stage`);
  if (!(RECOGNITION_FRONTIER_STAGES as readonly string[]).includes(stage)) {
    throw new Error(`${path}.stage: invalid stage`);
  }
  const relationId = relation ? item.relationId : null;
  if (relationId !== null && typeof relationId !== "string") {
    throw new Error(`${path}.relationId: must be text or null`);
  }
  return {
    decision,
    relationId,
    stage: stage as RecognitionFrontierStage,
  };
}

function isDecision(value: string): value is RecognitionDecision {
  return [
    "ambiguous",
    "conflicting",
    "established",
    "partial",
    "unsupported",
  ].includes(value);
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  path: string,
): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown) throw new Error(`${path}.${unknown}: unknown field`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path}: must be non-empty text`);
  }
  return value;
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`${path}: must be a non-negative integer`);
  }
  return value as number;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) {
    throw new Error(`${path}: values must be unique`);
  }
}
