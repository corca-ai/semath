import type { CorpusDocument } from "./model";

export const DOMAIN_CHALLENGE_FAMILIES = [
  "document-context",
  "section-scope",
  "multi-domain",
  "non-evidence",
  "formula-attachment",
  "conflict-and-retraction",
] as const;

export type DomainChallengeFamily = (typeof DOMAIN_CHALLENGE_FAMILIES)[number];
export type DomainChallengeSupport = "explicit" | "supported" | "tentative";

export interface DomainRoutingChallengeCase {
  readonly cursor: { readonly fileId: string; readonly needle: string };
  readonly documents: readonly CorpusDocument[];
  readonly expectedDecision: "ambiguous" | "conflicting" | "established" | "partial" | "unsupported";
  readonly expectedDomains: readonly { readonly packId: string; readonly support: DomainChallengeSupport }[];
  readonly expectedProblems: number;
  readonly family: DomainChallengeFamily;
  readonly id: string;
  readonly variationTags: readonly string[];
}

export interface DomainRoutingChallenge {
  readonly baseline: {
    readonly commit: string;
    readonly note: string;
    readonly protocolVersion: number;
  };
  readonly cases: readonly DomainRoutingChallengeCase[];
  readonly reviewedCollisionComponents: readonly {
    readonly componentId: string;
    readonly disposition: "covered" | "inapplicable";
    readonly reason: string;
  }[];
  readonly schemaVersion: 1;
}

export interface DomainRoutingObservation {
  readonly caseId: string;
  readonly decision: string;
  readonly domains: readonly { readonly packId: string; readonly support: string }[];
  readonly problemCount: number;
}

export function parseDomainRoutingChallenge(value: unknown): DomainRoutingChallenge {
  const root = record(value, "domain challenge");
  exact(root, ["schemaVersion", "baseline", "reviewedCollisionComponents", "cases"], "domain challenge");
  if (root.schemaVersion !== 1) throw new Error("domain challenge.schemaVersion: must be 1");
  const baseline = record(root.baseline, "domain challenge.baseline");
  exact(baseline, ["commit", "protocolVersion", "note"], "domain challenge.baseline");
  if (!Array.isArray(root.cases) || root.cases.length < 24) {
    throw new Error("domain challenge.cases: must contain at least 24 independently authored cases");
  }
  const cases = root.cases.map(parseCase);
  unique(cases.map((item) => item.id), "domain challenge.cases.id");
  for (const family of DOMAIN_CHALLENGE_FAMILIES) {
    if (cases.filter((item) => item.family === family).length < 4) {
      throw new Error(`domain challenge.cases: ${family} requires at least 4 cases`);
    }
  }
  if (!Array.isArray(root.reviewedCollisionComponents) || !root.reviewedCollisionComponents.length) {
    throw new Error("domain challenge.reviewedCollisionComponents: must not be empty");
  }
  const reviewedCollisionComponents = root.reviewedCollisionComponents.map((value, index) => {
    const path = `domain challenge.reviewedCollisionComponents[${index}]`;
    const item = record(value, path);
    exact(item, ["componentId", "disposition", "reason"], path);
    const disposition = text(item.disposition, `${path}.disposition`);
    if (disposition !== "covered" && disposition !== "inapplicable") {
      throw new Error(`${path}.disposition: invalid disposition`);
    }
    return {
      componentId: text(item.componentId, `${path}.componentId`),
      disposition: disposition as "covered" | "inapplicable",
      reason: text(item.reason, `${path}.reason`),
    };
  });
  unique(reviewedCollisionComponents.map((item) => item.componentId), "domain challenge.reviewedCollisionComponents.componentId");
  return {
    baseline: {
      commit: text(baseline.commit, "domain challenge.baseline.commit"),
      note: text(baseline.note, "domain challenge.baseline.note"),
      protocolVersion: integer(baseline.protocolVersion, "domain challenge.baseline.protocolVersion"),
    },
    cases,
    reviewedCollisionComponents,
    schemaVersion: 1,
  };
}

export function scoreDomainRoutingChallenge(
  challenge: DomainRoutingChallenge,
  observations: readonly DomainRoutingObservation[],
): { readonly cases: number; readonly failures: readonly string[]; readonly passed: number } {
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
    const prefix = observed.domains.slice(0, item.expectedDomains.length);
    if (JSON.stringify(prefix) !== JSON.stringify(item.expectedDomains)) {
      failures.push(`${item.id}: domains ${JSON.stringify(prefix)}; expected ${JSON.stringify(item.expectedDomains)}`);
    }
    if (observed.problemCount !== item.expectedProblems) {
      failures.push(`${item.id}: problems ${observed.problemCount}; expected ${item.expectedProblems}`);
    }
  }
  return {
    cases: challenge.cases.length,
    failures,
    passed: challenge.cases.length - new Set(failures.map((failure) => failure.split(":", 1)[0])).size,
  };
}

function parseCase(value: unknown, index: number): DomainRoutingChallengeCase {
  const path = `domain challenge.cases[${index}]`;
  const item = record(value, path);
  exact(item, ["id", "family", "documents", "cursor", "expectedDomains", "expectedDecision", "expectedProblems", "variationTags"], path);
  const family = text(item.family, `${path}.family`);
  if (!(DOMAIN_CHALLENGE_FAMILIES as readonly string[]).includes(family)) throw new Error(`${path}.family: unknown family`);
  if (!Array.isArray(item.documents) || !item.documents.length) throw new Error(`${path}.documents: must not be empty`);
  const documents = item.documents.map((value, documentIndex) => {
    const documentPath = `${path}.documents[${documentIndex}]`;
    const document = record(value, documentPath);
    exact(document, ["fileId", "path", "content"], documentPath);
    return { fileId: text(document.fileId, `${documentPath}.fileId`), path: text(document.path, `${documentPath}.path`), content: text(document.content, `${documentPath}.content`) };
  });
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(cursor, ["fileId", "needle"], `${path}.cursor`);
  if (!Array.isArray(item.expectedDomains)) throw new Error(`${path}.expectedDomains: must be an array`);
  const expectedDomains = item.expectedDomains.map((value, domainIndex) => {
    const domainPath = `${path}.expectedDomains[${domainIndex}]`;
    const domain = record(value, domainPath);
    exact(domain, ["packId", "support"], domainPath);
    const support = text(domain.support, `${domainPath}.support`);
    if (!(["explicit", "supported", "tentative"] as const).includes(support as DomainChallengeSupport)) throw new Error(`${domainPath}.support: invalid support`);
    return { packId: text(domain.packId, `${domainPath}.packId`), support: support as DomainChallengeSupport };
  });
  const decision = text(item.expectedDecision, `${path}.expectedDecision`);
  if (!(["ambiguous", "conflicting", "established", "partial", "unsupported"] as const).includes(decision as DomainRoutingChallengeCase["expectedDecision"])) throw new Error(`${path}.expectedDecision: invalid decision`);
  if (!Array.isArray(item.variationTags) || !item.variationTags.length) throw new Error(`${path}.variationTags: must not be empty`);
  return {
    cursor: { fileId: text(cursor.fileId, `${path}.cursor.fileId`), needle: text(cursor.needle, `${path}.cursor.needle`) },
    documents,
    expectedDecision: decision as DomainRoutingChallengeCase["expectedDecision"],
    expectedDomains,
    expectedProblems: integer(item.expectedProblems, `${path}.expectedProblems`),
    family: family as DomainChallengeFamily,
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
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error(`${path}: must be a non-negative integer`);
  return value as number;
}
function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: values must be unique`);
}
