import type { CorpusDocument } from "./model";

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
  readonly documents: readonly CorpusDocument[];
  readonly expectedDecision: "established" | "partial" | "unsupported";
  readonly expectedRelationId: string | null;
  readonly family: EquivalenceFamily;
  readonly id: string;
  readonly variationTags: readonly string[];
}

export interface EquivalenceChallenge {
  readonly baseline: {
    readonly commit: string;
    readonly passed: number;
    readonly total: number;
  };
  readonly cases: readonly EquivalenceChallengeCase[];
  readonly schemaVersion: 1;
}

export interface EquivalenceObservation {
  readonly caseId: string;
  readonly decision: string;
  readonly problemCount: number;
  readonly relationId: string | null;
}

export interface EquivalenceScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly firstFailure?: string;
  readonly passed: number;
}

export function parseEquivalenceChallenge(value: unknown): EquivalenceChallenge {
  const root = record(value, "equivalence challenge");
  exact(root, ["schemaVersion", "baseline", "cases"], "equivalence challenge");
  if (root.schemaVersion !== 1) throw new Error("equivalence challenge.schemaVersion: must be 1");
  const baseline = record(root.baseline, "equivalence challenge.baseline");
  exact(baseline, ["commit", "passed", "total"], "equivalence challenge.baseline");
  if (!Array.isArray(root.cases) || root.cases.length < 24) {
    throw new Error("equivalence challenge.cases: must contain at least 24 frozen cases");
  }
  const cases = root.cases.map((item, index) => parseCase(item, index));
  unique(cases.map((item) => item.id), "equivalence challenge.cases.id");
  for (const family of EQUIVALENCE_FAMILIES) {
    if (cases.filter((item) => item.family === family).length < 4) {
      throw new Error(`equivalence challenge.cases: ${family} requires at least 4 cases`);
    }
  }
  const total = integer(baseline.total, "equivalence challenge.baseline.total");
  const passed = integer(baseline.passed, "equivalence challenge.baseline.passed");
  if (total !== cases.length || passed < 0 || passed > total) {
    throw new Error("equivalence challenge.baseline: invalid score");
  }
  return {
    baseline: {
      commit: text(baseline.commit, "equivalence challenge.baseline.commit"),
      passed,
      total,
    },
    cases,
    schemaVersion: 1,
  };
}

export function scoreEquivalenceChallenge(
  challenge: EquivalenceChallenge,
  observations: readonly EquivalenceObservation[],
): EquivalenceScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  if (byId.size !== observations.length) failures.push("duplicate observations");
  for (const item of challenge.cases) {
    const observed = byId.get(item.id);
    if (!observed) {
      failures.push(`${item.id}: missing observation`);
      continue;
    }
    if (observed.decision !== item.expectedDecision) {
      failures.push(`${item.id}: decision ${observed.decision}; expected ${item.expectedDecision}`);
    }
    if (observed.relationId !== item.expectedRelationId) {
      failures.push(`${item.id}: relation ${observed.relationId}; expected ${item.expectedRelationId}`);
    }
    if (observed.problemCount !== 0) {
      failures.push(`${item.id}: exposed ${observed.problemCount} user problem(s)`);
    }
  }
  return {
    cases: challenge.cases.length,
    failures,
    ...(failures[0] ? { firstFailure: failures[0] } : {}),
    passed: challenge.cases.length - new Set(failures.map((failure) => failure.split(":", 1)[0])).size,
  };
}

function parseCase(value: unknown, index: number): EquivalenceChallengeCase {
  const path = `equivalence challenge.cases[${index}]`;
  const item = record(value, path);
  exact(item, ["id", "family", "documents", "cursor", "expectedDecision", "expectedRelationId", "variationTags"], path);
  const family = text(item.family, `${path}.family`);
  if (!(EQUIVALENCE_FAMILIES as readonly string[]).includes(family)) {
    throw new Error(`${path}.family: unknown family ${family}`);
  }
  const decision = text(item.expectedDecision, `${path}.expectedDecision`);
  if (!["established", "partial", "unsupported"].includes(decision)) {
    throw new Error(`${path}.expectedDecision: invalid decision`);
  }
  if (item.expectedRelationId !== null && typeof item.expectedRelationId !== "string") {
    throw new Error(`${path}.expectedRelationId: must be a string or null`);
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
    documents,
    expectedDecision: decision as EquivalenceChallengeCase["expectedDecision"],
    expectedRelationId: item.expectedRelationId,
    family: family as EquivalenceFamily,
    id: text(item.id, `${path}.id`),
    variationTags: item.variationTags.map((tag, tagIndex) => text(tag, `${path}.variationTags[${tagIndex}]`)),
  };
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

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value)) throw new Error(`${path}: must be an integer`);
  return value as number;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: values must be unique`);
}
