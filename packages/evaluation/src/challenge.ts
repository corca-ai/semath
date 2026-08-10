import type { CorpusDocument } from "./model";

export const CHALLENGE_LAYERS = [
  "binding",
  "constraint",
  "pack",
  "presentation",
  "resolution",
  "syntax",
] as const;
export const CHALLENGE_METRICS = [
  "association",
  "constraint",
  "evidence",
  "navigation",
  "recognition",
  "refusal",
  "scope",
  "structure",
] as const;

export type ChallengeLayer = (typeof CHALLENGE_LAYERS)[number];
export type ChallengeMetric = (typeof CHALLENGE_METRICS)[number];
export type ChallengeOutcome = "positive" | "refusal";

export interface ChallengeExpectation {
  readonly assumptionValue?: string;
  readonly candidateFamily?: string;
  readonly candidateInterpretation?: string;
  readonly conceptId?: string;
  readonly definitionDescription?: string;
  readonly definitionRuleId?: string;
  readonly excludedConceptId?: string;
  readonly excludedCandidateFamily?: string;
  readonly excludedDefinitionSymbol?: string;
  readonly excludedRelationId?: string;
  readonly relationId?: string;
  readonly shape?: string;
  readonly sourceNotation?: string;
  readonly status?: string;
  readonly symbol?: string;
}

export interface ChallengeCase {
  readonly cursor: {
    readonly edge?: "after" | "before";
    readonly fileId: string;
    readonly needle: string;
  };
  readonly documents: readonly CorpusDocument[];
  readonly expectation: ChallengeExpectation;
  readonly id: string;
  readonly metric: ChallengeMetric;
  readonly outcome: ChallengeOutcome;
  readonly owner: ChallengeLayer;
  readonly variationTags: readonly string[];
}

export interface ChallengeCorpus {
  readonly cases: readonly ChallengeCase[];
  readonly schemaVersion: 2;
}

export interface ChallengeObservation {
  readonly assumptionValues: readonly string[];
  readonly candidates: readonly {
    readonly family: string;
    readonly interpretation: string;
  }[];
  readonly caseId: string;
  readonly conceptIds: readonly string[];
  readonly definitions: readonly {
    readonly description: string;
    readonly ruleId: string;
    readonly symbol: string;
  }[];
  readonly relationIds: readonly string[];
  readonly shapes: readonly string[];
  readonly sourceNotation?: string;
  readonly status?: string;
  readonly symbols: readonly string[];
}

export interface ChallengeScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly layers: Readonly<Record<ChallengeLayer, { passed: number; total: number }>>;
  readonly metrics: Readonly<Record<ChallengeMetric, { passed: number; total: number }>>;
  readonly outcomes: Readonly<Record<ChallengeOutcome, { passed: number; total: number }>>;
  readonly passed: number;
  readonly schemaVersion: 2;
}

export interface DevelopmentFixtureCase {
  readonly documents: readonly CorpusDocument[];
  readonly id: string;
}

export function findChallengeFixtureLeaks(
  challenge: readonly ChallengeCase[],
  development: readonly DevelopmentFixtureCase[],
): readonly string[] {
  const developmentIds = new Set(development.map((item) => item.id));
  const developmentSources = new Set(
    development.flatMap((item) =>
      item.documents.map((document) => normalizedSource(document.content)),
    ),
  );
  const leaks = new Set<string>();
  for (const item of challenge) {
    if (developmentIds.has(item.id)) leaks.add(`${item.id}: duplicate fixture id`);
    for (const document of item.documents) {
      if (developmentSources.has(normalizedSource(document.content))) {
        leaks.add(`${item.id}: duplicate fixture source`);
      }
    }
  }
  return [...leaks].sort();
}

export function parseChallengeCorpus(value: unknown): ChallengeCorpus {
  const root = record(value, "challenge");
  exact(root, ["schemaVersion", "cases"], "challenge");
  if (root.schemaVersion !== 2) throw new Error("challenge.schemaVersion: must be 2");
  if (!Array.isArray(root.cases) || root.cases.length < 48) {
    throw new Error("challenge.cases: must contain at least 48 frozen cases");
  }
  const cases = root.cases.map((item, index) => parseCase(item, `challenge.cases[${index}]`));
  unique(cases.map((item) => item.id), "challenge.cases.id");
  for (const layer of CHALLENGE_LAYERS) {
    for (const outcome of ["positive", "refusal"] as const) {
      if (!cases.some((item) => item.owner === layer && item.outcome === outcome)) {
        throw new Error(`challenge.cases: missing ${layer}/${outcome}`);
      }
    }
  }
  for (const metric of CHALLENGE_METRICS) {
    if (!cases.some((item) => item.metric === metric)) {
      throw new Error(`challenge.cases: missing metric ${metric}`);
    }
  }
  validateBoundaryPairs(cases);
  return { cases, schemaVersion: 2 };
}

export function scoreChallenge(
  corpus: ChallengeCorpus,
  observations: readonly ChallengeObservation[],
): ChallengeScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  if (byId.size !== observations.length) failures.push("challenge: duplicate observations");
  const passed = new Set<string>();
  for (const item of corpus.cases) {
    const observation = byId.get(item.id);
    if (!observation) {
      failures.push(`${item.id}: missing observation`);
      continue;
    }
    const expected = item.expectation;
    const mismatches = [
      expected.assumptionValue && !observation.assumptionValues.includes(expected.assumptionValue)
        ? `assumption ${expected.assumptionValue}`
        : undefined,
      expected.candidateFamily &&
      !observation.candidates.some(
        (candidate) =>
          candidate.family === expected.candidateFamily &&
          (!expected.candidateInterpretation ||
            candidate.interpretation === expected.candidateInterpretation),
      )
        ? `candidate ${expected.candidateFamily}/${expected.candidateInterpretation ?? "*"}`
        : undefined,
      expected.conceptId && !observation.conceptIds.includes(expected.conceptId)
        ? `concept ${expected.conceptId}`
        : undefined,
      expected.excludedCandidateFamily &&
      observation.candidates.some(
        (candidate) => candidate.family === expected.excludedCandidateFamily,
      )
        ? `excluded candidate ${expected.excludedCandidateFamily}`
        : undefined,
      expected.definitionDescription &&
      !observation.definitions.some(
        (definition) =>
          definition.description === expected.definitionDescription &&
          (!expected.symbol || definition.symbol === expected.symbol),
      )
        ? `definition ${expected.symbol ?? "*"}/${expected.definitionDescription}`
        : undefined,
      expected.definitionRuleId &&
      !observation.definitions.some(
        (definition) =>
          definition.ruleId === expected.definitionRuleId &&
          (!expected.symbol || definition.symbol === expected.symbol),
      )
        ? `definition evidence ${expected.definitionRuleId}`
        : undefined,
      expected.excludedConceptId && observation.conceptIds.includes(expected.excludedConceptId)
        ? `excluded concept ${expected.excludedConceptId}`
        : undefined,
      expected.excludedDefinitionSymbol &&
      observation.definitions.some(
        (definition) => definition.symbol === expected.excludedDefinitionSymbol,
      )
        ? `excluded definition ${expected.excludedDefinitionSymbol}`
        : undefined,
      expected.excludedRelationId && observation.relationIds.includes(expected.excludedRelationId)
        ? `excluded relation ${expected.excludedRelationId}`
        : undefined,
      expected.relationId && !observation.relationIds.includes(expected.relationId)
        ? `relation ${expected.relationId}`
        : undefined,
      expected.shape && !observation.shapes.includes(expected.shape)
        ? `shape ${expected.shape}`
        : undefined,
      expected.sourceNotation && observation.sourceNotation !== expected.sourceNotation
        ? `source notation ${expected.sourceNotation}`
        : undefined,
      expected.status && observation.status !== expected.status
        ? `status ${expected.status}`
        : undefined,
      expected.symbol && !observation.symbols.includes(expected.symbol)
        ? `symbol ${expected.symbol}`
        : undefined,
    ].filter((item): item is string => Boolean(item));
    if (mismatches.length) failures.push(`${item.id}: missing ${mismatches.join(", ")}`);
    else passed.add(item.id);
  }
  for (const item of observations) {
    if (!corpus.cases.some((candidate) => candidate.id === item.caseId)) {
      failures.push(`${item.caseId}: unexpected observation`);
    }
  }
  return {
    cases: corpus.cases.length,
    failures: [...new Set(failures)].sort(),
    layers: tally(corpus.cases, passed, CHALLENGE_LAYERS, (item) => item.owner),
    metrics: tally(corpus.cases, passed, CHALLENGE_METRICS, (item) => item.metric),
    outcomes: tally(
      corpus.cases,
      passed,
      ["positive", "refusal"] as const,
      (item) => item.outcome,
    ),
    passed: passed.size,
    schemaVersion: 2,
  };
}

function parseCase(value: unknown, path: string): ChallengeCase {
  const item = record(value, path);
  exact(
    item,
    ["id", "documents", "cursor", "expectation", "metric", "outcome", "owner", "variationTags"],
    path,
  );
  const id = text(item.id, `${path}.id`);
  if (!Array.isArray(item.documents) || !item.documents.length) {
    throw new Error(`${path}.documents: must be a nonempty array`);
  }
  const documents = item.documents.map((value, index) => {
    const document = record(value, `${path}.documents[${index}]`);
    exact(document, ["content", "fileId", "path"], `${path}.documents[${index}]`);
    return {
      content: text(document.content, `${path}.documents[${index}].content`),
      fileId: text(document.fileId, `${path}.documents[${index}].fileId`),
      path: text(document.path, `${path}.documents[${index}].path`),
    };
  });
  unique(documents.map((item) => item.fileId), `${path}.documents.fileId`);
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(cursor, ["edge", "fileId", "needle"], `${path}.cursor`);
  const cursorFileId = text(cursor.fileId, `${path}.cursor.fileId`);
  const needle = text(cursor.needle, `${path}.cursor.needle`);
  const cursorDocument = documents.find((document) => document.fileId === cursorFileId);
  if (!cursorDocument) throw new Error(`${path}.cursor.fileId: unknown ${cursorFileId}`);
  const occurrences = cursorDocument.content.split(needle).length - 1;
  if (occurrences !== 1) {
    throw new Error(`${path}.cursor.needle: must occur exactly once; found ${occurrences}`);
  }
  const expectation = record(item.expectation, `${path}.expectation`);
  const expectationKeys = [
    "assumptionValue",
    "candidateFamily",
    "candidateInterpretation",
    "conceptId",
    "excludedCandidateFamily",
    "definitionDescription",
    "definitionRuleId",
    "excludedConceptId",
    "excludedDefinitionSymbol",
    "excludedRelationId",
    "relationId",
    "shape",
    "sourceNotation",
    "status",
    "symbol",
  ] as const;
  exact(expectation, expectationKeys, `${path}.expectation`);
  if (!Object.keys(expectation).length) {
    throw new Error(`${path}.expectation: must not be empty`);
  }
  return {
    cursor: {
      ...(cursor.edge === undefined
        ? {}
        : { edge: oneOf(cursor.edge, ["after", "before"] as const, `${path}.cursor.edge`) }),
      fileId: cursorFileId,
      needle,
    },
    documents,
    expectation: Object.fromEntries(
      expectationKeys.flatMap((key) =>
        expectation[key] === undefined
          ? []
          : [[key, text(expectation[key], `${path}.expectation.${key}`)]],
      ),
    ),
    id,
    metric: oneOf(item.metric, CHALLENGE_METRICS, `${path}.metric`),
    outcome: oneOf(item.outcome, ["positive", "refusal"] as const, `${path}.outcome`),
    owner: oneOf(item.owner, CHALLENGE_LAYERS, `${path}.owner`),
    variationTags: stringList(item.variationTags, `${path}.variationTags`),
  };
}

function validateBoundaryPairs(cases: readonly ChallengeCase[]): void {
  const pairs = new Map<string, Set<ChallengeOutcome>>();
  for (const item of cases) {
    for (const tag of item.variationTags) {
      if (!tag.startsWith("boundary-pair:")) continue;
      const pair = tag.slice("boundary-pair:".length);
      if (!pair) throw new Error(`${item.id}: boundary pair must have an id`);
      const outcomes = pairs.get(pair) ?? new Set<ChallengeOutcome>();
      outcomes.add(item.outcome);
      pairs.set(pair, outcomes);
    }
  }
  if (pairs.size < 12) {
    throw new Error("challenge.cases: must contain at least 12 semantic boundary pairs");
  }
  for (const [pair, outcomes] of pairs) {
    if (!outcomes.has("positive") || !outcomes.has("refusal")) {
      throw new Error(`challenge.cases: incomplete boundary pair ${pair}`);
    }
  }
}

function normalizedSource(source: string): string {
  return source.replace(/\s+/gu, " ").trim();
}

function tally<
  const Keys extends readonly string[],
  Item extends { readonly id: string },
>(
  items: readonly Item[],
  passed: ReadonlySet<string>,
  keys: Keys,
  select: (item: Item) => Keys[number],
): Record<Keys[number], { passed: number; total: number }> {
  return Object.fromEntries(
    keys.map((key) => {
      const selected = items.filter((item) => select(item) === key);
      return [
        key,
        { passed: selected.filter((item) => passed.has(item.id)).length, total: selected.length },
      ];
    }),
  ) as Record<Keys[number], { passed: number; total: number }>;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function exact(value: Record<string, unknown>, keys: readonly string[], path: string): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (unknown.length) throw new Error(`${path}: unknown field ${unknown.sort()[0]}`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${path}: must be text`);
  return value;
}

function stringList(value: unknown, path: string): string[] {
  if (!Array.isArray(value) || !value.length) throw new Error(`${path}: must be nonempty text[]`);
  const output = value.map((item, index) => text(item, `${path}[${index}]`));
  unique(output, path);
  return output;
}

function oneOf<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
  path: string,
): Values[number] {
  if (typeof value !== "string" || !values.includes(value)) {
    throw new Error(`${path}: must be one of ${values.join(", ")}`);
  }
  return value;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: must be unique`);
}
