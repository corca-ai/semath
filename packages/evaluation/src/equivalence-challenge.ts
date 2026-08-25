import type { CorpusDocument } from "./model";

export const EQUIVALENCE_DECISION_DOMAINS = [
  "cursor-entity",
  "selected-formula",
] as const;

export type EquivalenceDecisionDomain =
  (typeof EQUIVALENCE_DECISION_DOMAINS)[number];

export type EquivalenceDecision =
  | "established"
  | "partial"
  | "conventional"
  | "ambiguous"
  | "conflicting"
  | "unsupported"
  | "engine-limited";

export const EQUIVALENCE_FAMILIES = [
  "orientation",
  "scalar-permutation",
  "factor-isolation",
  "reciprocal",
  "grouping",
  "refusal",
] as const;

export type EquivalenceFamily = (typeof EQUIVALENCE_FAMILIES)[number];

export interface EquivalenceChallengeCase {
  readonly cursor: { readonly fileId: string; readonly needle: string };
  readonly decisionDomain: EquivalenceDecisionDomain;
  readonly documents: readonly CorpusDocument[];
  readonly expectedDecision: EquivalenceDecision;
  readonly expectedRelationId: string | null;
  readonly family: EquivalenceFamily;
  readonly id: string;
  readonly variationTags: readonly string[];
}

export interface EquivalenceChallenge {
  readonly lineage: {
    readonly previousCommit: string;
    readonly previousSchemaVersion: 1;
  };
  readonly cases: readonly EquivalenceChallengeCase[];
  readonly schemaVersion: 2;
}

export interface EquivalenceObservation {
  readonly caseId: string;
  readonly decision: EquivalenceDecision;
  readonly decisionDomain: EquivalenceDecisionDomain;
  readonly problemCount: number;
  readonly relationIds: readonly string[];
}

export interface EquivalenceDomainObservation {
  readonly decision: EquivalenceDecision;
  readonly problemCount: number;
  readonly relationIds: readonly string[];
}

export interface EquivalenceScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly firstFailure?: string;
  readonly passed: number;
}

export function parseEquivalenceChallenge(value: unknown): EquivalenceChallenge {
  const root = record(value, "equivalence challenge");
  exact(root, ["schemaVersion", "lineage", "cases"], "equivalence challenge");
  if (root.schemaVersion !== 2) throw new Error("equivalence challenge.schemaVersion: must be 2");
  const lineage = record(root.lineage, "equivalence challenge.lineage");
  exact(lineage, ["previousCommit", "previousSchemaVersion"], "equivalence challenge.lineage");
  if (lineage.previousSchemaVersion !== 1) {
    throw new Error("equivalence challenge.lineage.previousSchemaVersion: must be 1");
  }
  if (!Array.isArray(root.cases) || root.cases.length !== 24) {
    throw new Error("equivalence challenge.cases: must contain exactly 24 frozen cases");
  }
  const cases = root.cases.map((item, index) => parseCase(item, index));
  unique(cases.map((item) => item.id), "equivalence challenge.cases.id");
  for (const family of EQUIVALENCE_FAMILIES) {
    if (cases.filter((item) => item.family === family).length < 4) {
      throw new Error(`equivalence challenge.cases: ${family} requires at least 4 cases`);
    }
  }
  return {
    lineage: {
      previousCommit: text(lineage.previousCommit, "equivalence challenge.lineage.previousCommit"),
      previousSchemaVersion: 1,
    },
    cases,
    schemaVersion: 2,
  };
}

export function scoreEquivalenceChallenge(
  challenge: EquivalenceChallenge,
  observations: readonly EquivalenceObservation[],
): EquivalenceScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  if (byId.size !== observations.length) failures.push("duplicate observations");
  if (observations.length !== challenge.cases.length) {
    failures.push(`observation count ${observations.length}; expected ${challenge.cases.length}`);
  }
  const expectedIds = new Set(challenge.cases.map((item) => item.id));
  for (const observed of observations) {
    if (!expectedIds.has(observed.caseId)) {
      failures.push(`${observed.caseId}: unexpected observation`);
    }
  }
  for (const item of challenge.cases) {
    const observed = byId.get(item.id);
    if (!observed) {
      failures.push(`${item.id}: missing observation`);
      continue;
    }
    if (observed.decision !== item.expectedDecision) {
      failures.push(`${item.id}: decision ${observed.decision}; expected ${item.expectedDecision}`);
    }
    if (observed.decisionDomain !== item.decisionDomain) {
      failures.push(`${item.id}: decision domain ${observed.decisionDomain}; expected ${item.decisionDomain}`);
    }
    const expectedRelationIds = item.expectedRelationId ? [item.expectedRelationId] : [];
    if (!sameStrings(observed.relationIds, expectedRelationIds)) {
      failures.push(`${item.id}: relations ${observed.relationIds.join(",") || "none"}; expected ${expectedRelationIds.join(",") || "none"}`);
    }
    if (observed.problemCount !== 0) {
      failures.push(`${item.id}: exposed ${observed.problemCount} user problem(s)`);
    }
  }
  return {
    cases: challenge.cases.length,
    failures,
    ...(failures[0] ? { firstFailure: failures[0] } : {}),
    passed:
      challenge.cases.length -
      challenge.cases.filter((item) =>
        failures.some((failure) => failure.startsWith(`${item.id}:`)),
      ).length,
  };
}

export function selectEquivalenceObservation(
  caseId: string,
  decisionDomain: EquivalenceDecisionDomain,
  cursorEntity: EquivalenceDomainObservation,
  selectedFormula: EquivalenceDomainObservation,
): EquivalenceObservation {
  const selected = decisionDomain === "cursor-entity" ? cursorEntity : selectedFormula;
  return {
    caseId,
    decision: selected.decision,
    decisionDomain,
    problemCount: selected.problemCount,
    relationIds: [...new Set(selected.relationIds)].sort(),
  };
}

function parseCase(value: unknown, index: number): EquivalenceChallengeCase {
  const path = `equivalence challenge.cases[${index}]`;
  const item = record(value, path);
  exact(item, ["id", "family", "documents", "cursor", "decisionDomain", "expectedDecision", "expectedRelationId", "variationTags"], path);
  const family = text(item.family, `${path}.family`);
  if (!(EQUIVALENCE_FAMILIES as readonly string[]).includes(family)) {
    throw new Error(`${path}.family: unknown family ${family}`);
  }
  const decision = text(item.expectedDecision, `${path}.expectedDecision`);
  if (!["established", "partial", "conventional", "ambiguous", "conflicting", "unsupported", "engine-limited"].includes(decision)) {
    throw new Error(`${path}.expectedDecision: invalid decision`);
  }
  const decisionDomain = text(item.decisionDomain, `${path}.decisionDomain`);
  if (!(EQUIVALENCE_DECISION_DOMAINS as readonly string[]).includes(decisionDomain)) {
    throw new Error(`${path}.decisionDomain: invalid decision domain`);
  }
  if (decisionDomain === "cursor-entity" && (decision === "conventional" || decision === "engine-limited")) {
    throw new Error(`${path}.expectedDecision: invalid cursor-entity decision`);
  }
  const expectedRelationId =
    item.expectedRelationId === null
      ? null
      : text(item.expectedRelationId, `${path}.expectedRelationId`);
  if (item.expectedRelationId !== null && decisionDomain !== "selected-formula") {
    throw new Error(`${path}.expectedRelationId: relation expectations require selected-formula domain`);
  }
  if (!Array.isArray(item.documents) || !item.documents.length) throw new Error(`${path}.documents: must not be empty`);
  const documents = item.documents.map((value, documentIndex) => {
    const documentPath = `${path}.documents[${documentIndex}]`;
    const document = record(value, documentPath);
    exact(document, ["fileId", "path", "content"], documentPath);
    return { fileId: text(document.fileId, `${documentPath}.fileId`), path: text(document.path, `${documentPath}.path`), content: text(document.content, `${documentPath}.content`) };
  });
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(cursor, ["fileId", "needle"], `${path}.cursor`);
  if (!Array.isArray(item.variationTags) || !item.variationTags.length) throw new Error(`${path}.variationTags: must not be empty`);
  return {
    cursor: { fileId: text(cursor.fileId, `${path}.cursor.fileId`), needle: text(cursor.needle, `${path}.cursor.needle`) },
    decisionDomain: decisionDomain as EquivalenceDecisionDomain,
    documents,
    expectedDecision: decision as EquivalenceDecision,
    expectedRelationId,
    family: family as EquivalenceFamily,
    id: text(item.id, `${path}.id`),
    variationTags: item.variationTags.map((tag, tagIndex) => text(tag, `${path}.variationTags[${tagIndex}]`)),
  };
}

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error(`${path}: must be an object`);
  return value as Record<string, unknown>;
}

function exact(value: Record<string, unknown>, keys: readonly string[], path: string): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown) throw new Error(`${path}.${unknown}: unknown field`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${path}: must be non-empty text`);
  return value;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: values must be unique`);
}
