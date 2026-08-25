import type { CorpusDocument } from "./model";
import type { ChallengeRecognizedRelation } from "./challenge";

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
export type DomainRoutingDecisionDomain = "cursor-entity" | "selected-formula";
export type DomainRoutingDecision =
  | "ambiguous"
  | "conflicting"
  | "conventional"
  | "engine-limited"
  | "established"
  | "partial"
  | "unsupported";

export interface DomainRoutingChallengeCase {
  readonly cursor: { readonly fileId: string; readonly needle: string };
  readonly decisionDomain: DomainRoutingDecisionDomain;
  readonly documents: readonly CorpusDocument[];
  readonly expectedDecision: DomainRoutingDecision;
  readonly expectedDomains: readonly { readonly packId: string; readonly support: DomainChallengeSupport }[];
  readonly expectedProblems: number;
  readonly expectedRelations: readonly ChallengeRecognizedRelation[];
  readonly expectedSourceGrounded: boolean;
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
  readonly schemaVersion: 2;
}

export interface DomainRoutingObservation {
  readonly caseId: string;
  readonly decision: DomainRoutingDecision;
  readonly decisionDomain: DomainRoutingDecisionDomain;
  readonly domains: readonly { readonly packId: string; readonly support: string }[];
  readonly problemCount: number;
  readonly recognizedRelations: readonly ChallengeRecognizedRelation[];
  readonly sourceGrounded: boolean;
}

export function parseDomainRoutingChallenge(value: unknown): DomainRoutingChallenge {
  const root = record(value, "domain challenge");
  exact(root, ["schemaVersion", "baseline", "reviewedCollisionComponents", "cases"], "domain challenge");
  if (root.schemaVersion !== 2) throw new Error("domain challenge.schemaVersion: must be 2");
  const baseline = record(root.baseline, "domain challenge.baseline");
  exact(baseline, ["commit", "protocolVersion", "note"], "domain challenge.baseline");
  if (!Array.isArray(root.cases) || root.cases.length !== 30) {
    throw new Error("domain challenge.cases: must contain exactly 30 independently authored cases");
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
    schemaVersion: 2,
  };
}

export function scoreDomainRoutingChallenge(
  challenge: DomainRoutingChallenge,
  observations: readonly DomainRoutingObservation[],
): { readonly cases: number; readonly failures: readonly string[]; readonly passed: number } {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  if (byId.size !== observations.length) failures.push("duplicate observations");
  const expectedIds = new Set(challenge.cases.map((item) => item.id));
  if (observations.length !== challenge.cases.length) {
    failures.push(`observation count ${observations.length}; expected ${challenge.cases.length}`);
  }
  for (const observation of observations) {
    if (!expectedIds.has(observation.caseId)) {
      failures.push(`${observation.caseId}: unexpected observation`);
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
    if (JSON.stringify(observed.domains) !== JSON.stringify(item.expectedDomains)) {
      failures.push(`${item.id}: domains ${JSON.stringify(observed.domains)}; expected ${JSON.stringify(item.expectedDomains)}`);
    }
    if (observed.problemCount !== item.expectedProblems) {
      failures.push(`${item.id}: problems ${observed.problemCount}; expected ${item.expectedProblems}`);
    }
    if (JSON.stringify(observed.recognizedRelations) !== JSON.stringify(item.expectedRelations)) {
      failures.push(`${item.id}: relations ${JSON.stringify(observed.recognizedRelations)}; expected ${JSON.stringify(item.expectedRelations)}`);
    }
    if (observed.sourceGrounded !== item.expectedSourceGrounded) {
      failures.push(`${item.id}: source grounded ${observed.sourceGrounded}; expected ${item.expectedSourceGrounded}`);
    }
  }
  return {
    cases: challenge.cases.length,
    failures,
    passed: challenge.cases.length - new Set(failures.map((failure) => failure.split(":", 1)[0])).size,
  };
}

export function selectDomainRoutingDecision(
  decisionDomain: DomainRoutingDecisionDomain,
  cursorDecision: DomainRoutingDecision,
  formulaDecision: DomainRoutingDecision,
): DomainRoutingDecision {
  return decisionDomain === "cursor-entity" ? cursorDecision : formulaDecision;
}

function parseCase(value: unknown, index: number): DomainRoutingChallengeCase {
  const path = `domain challenge.cases[${index}]`;
  const item = record(value, path);
  const selectedFormula = item.decisionDomain === "selected-formula";
  exact(item, ["id", "family", "documents", "cursor", "decisionDomain", "expectedDomains", "expectedDecision", "expectedProblems", ...(selectedFormula ? ["expectedRelations", "expectedSourceGrounded"] : []), "variationTags"], path);
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
  const decisionDomain = text(item.decisionDomain, `${path}.decisionDomain`);
  if (decisionDomain !== "cursor-entity" && decisionDomain !== "selected-formula") {
    throw new Error(`${path}.decisionDomain: invalid decision domain`);
  }
  const decision = text(item.expectedDecision, `${path}.expectedDecision`);
  if (!(["ambiguous", "conflicting", "conventional", "engine-limited", "established", "partial", "unsupported"] as const).includes(decision as DomainRoutingDecision)) throw new Error(`${path}.expectedDecision: invalid decision`);
  if (decisionDomain === "cursor-entity" && (decision === "conventional" || decision === "engine-limited")) {
    throw new Error(`${path}.expectedDecision: invalid cursor-entity decision`);
  }
  if (selectedFormula && !Array.isArray(item.expectedRelations)) {
    throw new Error(`${path}.expectedRelations: must be an array`);
  }
  const expectedRelations = (selectedFormula ? item.expectedRelations as unknown[] : []).map((value, relationIndex) => {
    const relationPath = `${path}.expectedRelations[${relationIndex}]`;
    const relation = record(value, relationPath);
    exact(relation, ["authority", "formulaAnchor", "relationId", "support"], relationPath);
    const authority = text(relation.authority, `${relationPath}.authority`);
    if (authority !== "authoritative" && authority !== "candidate") {
      throw new Error(`${relationPath}.authority: invalid authority`);
    }
    const support = text(relation.support, `${relationPath}.support`);
    if (!( ["explicit", "derived", "supported", "tentative"] as const).includes(support as never)) {
      throw new Error(`${relationPath}.support: invalid support`);
    }
    if (relation.formulaAnchor !== "selected-formula") {
      throw new Error(`${relationPath}.formulaAnchor: must be selected-formula`);
    }
    return {
      authority: authority as "authoritative" | "candidate",
      formulaAnchor: "selected-formula" as const,
      relationId: text(relation.relationId, `${relationPath}.relationId`),
      support: support as "explicit" | "derived" | "supported" | "tentative",
    };
  });
  if (selectedFormula !== (expectedRelations.length > 0)) {
    throw new Error(`${path}.expectedRelations: selected-formula cases require a nonempty exact relation set`);
  }
  if (selectedFormula && typeof item.expectedSourceGrounded !== "boolean") {
    throw new Error(`${path}.expectedSourceGrounded: must be boolean`);
  }
  if (!Array.isArray(item.variationTags) || !item.variationTags.length) throw new Error(`${path}.variationTags: must not be empty`);
  return {
    cursor: { fileId: text(cursor.fileId, `${path}.cursor.fileId`), needle: text(cursor.needle, `${path}.cursor.needle`) },
    decisionDomain: decisionDomain as DomainRoutingDecisionDomain,
    documents,
    expectedDecision: decision as DomainRoutingDecision,
    expectedDomains,
    expectedProblems: integer(item.expectedProblems, `${path}.expectedProblems`),
    expectedRelations,
    expectedSourceGrounded: selectedFormula ? item.expectedSourceGrounded as boolean : false,
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
