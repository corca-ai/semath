import type { CorpusDocument } from "./model";

export const SEMANTIC_CONTINUITY_FAMILIES = [
  "lifetime-shadowing",
  "notation-identity",
  "discourse-flow",
  "canonical-structure",
  "typed-propagation",
  "safety-retraction",
] as const;

export type SemanticContinuityFamily =
  (typeof SEMANTIC_CONTINUITY_FAMILIES)[number];
export type SemanticContinuityDecision =
  | "ambiguous"
  | "conflicting"
  | "established"
  | "partial"
  | "unsupported";

export interface SemanticContinuityExpectation {
  readonly decision: SemanticContinuityDecision;
  readonly definitionDescription?: string;
  readonly excludedDefinitionDescription?: string;
  readonly maximumProblems: number;
  readonly minimumProblems: number;
  readonly relationId?: string;
  readonly shapeKind?: string;
  readonly symbol?: string;
}

export interface SemanticContinuityCase {
  readonly baseline: {
    readonly decision: SemanticContinuityDecision;
    readonly problems: number;
  };
  readonly cursor: {
    readonly edge?: "after" | "before";
    readonly fileId: string;
    readonly needle: string;
  };
  readonly documents: readonly CorpusDocument[];
  readonly family: SemanticContinuityFamily;
  readonly id: string;
  readonly target: SemanticContinuityExpectation;
  readonly variationTags: readonly string[];
}

export interface SemanticContinuityFixture {
  readonly baseline: {
    readonly commit: string;
    readonly note: string;
    readonly protocolVersion: number;
  };
  readonly cases: readonly SemanticContinuityCase[];
  readonly schemaVersion: 1;
}

export interface SemanticContinuityObservation {
  readonly caseId: string;
  readonly decision: SemanticContinuityDecision;
  readonly definitions: readonly string[];
  readonly problems: number;
  readonly relationIds: readonly string[];
  readonly shapeKinds: readonly string[];
  readonly symbol: string | null;
}

export interface SemanticContinuityScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly families: Readonly<
    Record<SemanticContinuityFamily, { readonly passed: number; readonly total: number }>
  >;
  readonly passed: number;
  readonly risk: {
    readonly falseConflict: number;
    readonly falseEstablishment: number;
    readonly missedCoverage: number;
    readonly navigationOrIdentity: number;
    readonly total: number;
  };
}

export function parseSemanticContinuityFixture(
  value: unknown,
): SemanticContinuityFixture {
  const root = record(value, "semantic continuity");
  exact(root, ["schemaVersion", "baseline", "cases"], "semantic continuity");
  if (root.schemaVersion !== 1) {
    throw new Error("semantic continuity.schemaVersion: must be 1");
  }
  const baseline = record(root.baseline, "semantic continuity.baseline");
  exact(
    baseline,
    ["commit", "protocolVersion", "note"],
    "semantic continuity.baseline",
  );
  if (!Array.isArray(root.cases) || root.cases.length < 48) {
    throw new Error(
      "semantic continuity.cases: requires at least 48 independently authored cases",
    );
  }
  const cases = root.cases.map(parseCase);
  unique(cases.map((item) => item.id), "semantic continuity.cases.id");
  for (const family of SEMANTIC_CONTINUITY_FAMILIES) {
    if (cases.filter((item) => item.family === family).length < 8) {
      throw new Error(
        `semantic continuity.cases: ${family} requires at least 8 cases`,
      );
    }
  }
  const normalizedDocuments = cases.map((item) =>
    item.documents
      .map((document) =>
        document.content.toLowerCase().replaceAll(/\s+/gu, " ").trim(),
      )
      .join("\n"),
  );
  unique(normalizedDocuments, "semantic continuity normalized documents");
  return {
    baseline: {
      commit: text(baseline.commit, "semantic continuity.baseline.commit"),
      note: text(baseline.note, "semantic continuity.baseline.note"),
      protocolVersion: integer(
        baseline.protocolVersion,
        "semantic continuity.baseline.protocolVersion",
      ),
    },
    cases,
    schemaVersion: 1,
  };
}

export function scoreSemanticContinuity(
  fixture: SemanticContinuityFixture,
  observations: readonly SemanticContinuityObservation[],
): SemanticContinuityScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  const families = Object.fromEntries(
    SEMANTIC_CONTINUITY_FAMILIES.map((family) => [
      family,
      { passed: 0, total: 0 },
    ]),
  ) as Record<
    SemanticContinuityFamily,
    { passed: number; total: number }
  >;
  let falseConflict = 0;
  let falseEstablishment = 0;
  let missedCoverage = 0;
  let navigationOrIdentity = 0;
  if (byId.size !== observations.length) failures.push("duplicate observations");
  for (const item of fixture.cases) {
    families[item.family].total += 1;
    const observed = byId.get(item.id);
    if (!observed) {
      failures.push(`${item.id}: missing observation`);
      missedCoverage += 1;
      continue;
    }
    const caseFailures: string[] = [];
    let caseFalseConflict = false;
    let caseFalseEstablishment = false;
    let caseIdentityFailure = false;
    let caseMissedCoverage = false;
    if (observed.decision !== item.target.decision) {
      caseFailures.push(
        `decision ${observed.decision}; expected ${item.target.decision}`,
      );
      if (
        observed.decision === "established" &&
        item.target.decision !== "established"
      ) {
        caseFalseEstablishment = true;
      } else if (
        observed.decision === "conflicting" &&
        item.target.decision !== "conflicting"
      ) {
        caseFalseConflict = true;
      } else {
        caseMissedCoverage = true;
      }
    }
    if (
      observed.problems < item.target.minimumProblems ||
      observed.problems > item.target.maximumProblems
    ) {
      caseFailures.push(
        `problems ${observed.problems}; expected ${item.target.minimumProblems}..${item.target.maximumProblems}`,
      );
      if (observed.problems > item.target.maximumProblems) {
        caseFalseConflict = true;
      } else {
        caseMissedCoverage = true;
      }
    }
    if (item.target.symbol && observed.symbol !== item.target.symbol) {
      caseFailures.push(
        `symbol ${observed.symbol ?? "null"}; expected ${item.target.symbol}`,
      );
      caseIdentityFailure = true;
    }
    if (
      item.target.definitionDescription &&
      !observed.definitions.includes(item.target.definitionDescription)
    ) {
      caseFailures.push(
        `missing definition ${item.target.definitionDescription}`,
      );
      caseMissedCoverage = true;
    }
    if (
      item.target.excludedDefinitionDescription &&
      observed.definitions.includes(item.target.excludedDefinitionDescription)
    ) {
      caseFailures.push(
        `leaked definition ${item.target.excludedDefinitionDescription}`,
      );
      caseIdentityFailure = true;
    }
    if (
      item.target.relationId &&
      !observed.relationIds.includes(item.target.relationId)
    ) {
      caseFailures.push(`missing relation ${item.target.relationId}`);
      caseMissedCoverage = true;
    }
    if (
      item.target.shapeKind &&
      !observed.shapeKinds.includes(item.target.shapeKind)
    ) {
      caseFailures.push(`missing shape ${item.target.shapeKind}`);
      caseMissedCoverage = true;
    }
    falseConflict += Number(caseFalseConflict);
    falseEstablishment += Number(caseFalseEstablishment);
    navigationOrIdentity += Number(caseIdentityFailure);
    missedCoverage += Number(caseMissedCoverage);
    if (caseFailures.length) {
      failures.push(`${item.id}: ${caseFailures.join("; ")}`);
    } else {
      families[item.family].passed += 1;
    }
  }
  return {
    cases: fixture.cases.length,
    failures,
    families,
    passed:
      fixture.cases.length -
      new Set(failures.map((failure) => failure.split(":", 1)[0])).size,
    risk: {
      falseConflict,
      falseEstablishment,
      missedCoverage,
      navigationOrIdentity,
      total:
        falseConflict * 12 +
        falseEstablishment * 12 +
        navigationOrIdentity * 10 +
        missedCoverage * 2,
    },
  };
}

function parseCase(value: unknown, index: number): SemanticContinuityCase {
  const path = `semantic continuity.cases[${index}]`;
  const item = record(value, path);
  exact(
    item,
    ["id", "family", "documents", "cursor", "baseline", "target", "variationTags"],
    path,
  );
  const family = text(item.family, `${path}.family`);
  if (!(SEMANTIC_CONTINUITY_FAMILIES as readonly string[]).includes(family)) {
    throw new Error(`${path}.family: unknown family`);
  }
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
  exact(cursor, ["fileId", "needle", "edge"], `${path}.cursor`);
  const edge = cursor.edge === undefined ? undefined : text(cursor.edge, `${path}.cursor.edge`);
  if (edge !== undefined && edge !== "before" && edge !== "after") {
    throw new Error(`${path}.cursor.edge: invalid edge`);
  }
  const baseline = record(item.baseline, `${path}.baseline`);
  exact(baseline, ["decision", "problems"], `${path}.baseline`);
  const target = parseExpectation(item.target, `${path}.target`);
  if (!Array.isArray(item.variationTags) || item.variationTags.length < 2) {
    throw new Error(`${path}.variationTags: requires at least two tags`);
  }
  return {
    baseline: {
      decision: decision(baseline.decision, `${path}.baseline.decision`),
      problems: integer(baseline.problems, `${path}.baseline.problems`),
    },
    cursor: {
      ...(edge ? { edge } : {}),
      fileId: text(cursor.fileId, `${path}.cursor.fileId`),
      needle: text(cursor.needle, `${path}.cursor.needle`),
    },
    documents,
    family: family as SemanticContinuityFamily,
    id: text(item.id, `${path}.id`),
    target,
    variationTags: item.variationTags.map((tag, tagIndex) =>
      text(tag, `${path}.variationTags[${tagIndex}]`),
    ),
  };
}

function parseExpectation(
  value: unknown,
  path: string,
): SemanticContinuityExpectation {
  const item = record(value, path);
  exact(
    item,
    [
      "decision",
      "minimumProblems",
      "maximumProblems",
      "symbol",
      "definitionDescription",
      "excludedDefinitionDescription",
      "relationId",
      "shapeKind",
    ],
    path,
  );
  const minimumProblems = integer(item.minimumProblems, `${path}.minimumProblems`);
  const maximumProblems = integer(item.maximumProblems, `${path}.maximumProblems`);
  if (minimumProblems > maximumProblems) {
    throw new Error(`${path}: minimumProblems exceeds maximumProblems`);
  }
  return {
    decision: decision(item.decision, `${path}.decision`),
    ...(item.definitionDescription === undefined
      ? {}
      : {
          definitionDescription: text(
            item.definitionDescription,
            `${path}.definitionDescription`,
          ),
        }),
    ...(item.excludedDefinitionDescription === undefined
      ? {}
      : {
          excludedDefinitionDescription: text(
            item.excludedDefinitionDescription,
            `${path}.excludedDefinitionDescription`,
          ),
        }),
    maximumProblems,
    minimumProblems,
    ...(item.relationId === undefined
      ? {}
      : { relationId: text(item.relationId, `${path}.relationId`) }),
    ...(item.shapeKind === undefined
      ? {}
      : { shapeKind: text(item.shapeKind, `${path}.shapeKind`) }),
    ...(item.symbol === undefined
      ? {}
      : { symbol: text(item.symbol, `${path}.symbol`) }),
  };
}

function decision(value: unknown, path: string): SemanticContinuityDecision {
  const result = text(value, path);
  if (
    ![
      "ambiguous",
      "conflicting",
      "established",
      "partial",
      "unsupported",
    ].includes(result)
  ) {
    throw new Error(`${path}: invalid decision`);
  }
  return result as SemanticContinuityDecision;
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
  const missing = keys.find(
    (key) =>
      !Object.hasOwn(value, key) &&
      ![
        "edge",
        "symbol",
        "definitionDescription",
        "excludedDefinitionDescription",
        "relationId",
        "shapeKind",
      ].includes(key),
  );
  if (missing) throw new Error(`${path}.${missing}: missing field`);
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
