export interface CorpusDocument {
  content: string;
  fileId: string;
  path: string;
}

export interface FoundationExpectation {
  assumptionKind?: string;
  assumptionSubject?: string;
  assumptionValue?: string;
  conceptId?: string;
  definitionDescription?: string;
  definitionEvidenceRuleId?: string;
  diagnosticCode?: string;
  dimension?: string;
  excludedConceptId?: string;
  excludedDefinitionSymbol?: string;
  excludedAssumptionValue?: string;
  excludedQuantityKindId?: string;
  excludedRelationId?: string;
  quantityKindId?: string;
  relationId?: string;
  status?: string;
  symbol?: string;
  unitId?: string;
}

export type FoundationMetric =
  | "association"
  | "assumption"
  | "classification"
  | "evidence"
  | "refusal"
  | "scope";

export interface FoundationCase {
  cursor: { edge?: "after" | "before"; fileId: string; needle: string };
  documents: readonly CorpusDocument[];
  expectation: FoundationExpectation;
  id: string;
  metric?: FoundationMetric;
  variationTags: readonly string[];
}

export interface FoundationCorpus {
  cases: readonly FoundationCase[];
  domain: string;
  schemaVersion: 1;
}

export interface FoundationObservation {
  assumptions: readonly {
    kind: string;
    subjects: readonly string[];
    value: string;
  }[];
  caseId: string;
  conceptIds: readonly string[];
  diagnosticCodes: readonly string[];
  definitions: readonly {
    description: string;
    evidenceRuleIds: readonly string[];
    symbol: string;
  }[];
  dimensions: readonly string[];
  quantityKindIds: readonly string[];
  relationIds: readonly string[];
  status?: string;
  suiteId: string;
  symbols: readonly string[];
  unitIds: readonly string[];
}

export interface FoundationScorecard {
  cases: number;
  failures: readonly string[];
  metrics: Readonly<Record<string, { cases: number; passed: number }>>;
  passed: number;
  schemaVersion: 1;
}

export function parseFoundationCorpus(value: unknown): FoundationCorpus {
  const root = record(value, "foundation");
  const suiteId = identifier(root.domain, "foundation.domain");
  exact(root, ["schemaVersion", "domain", "cases"], `foundation ${suiteId}`);
  if (root.schemaVersion !== 1) throw new Error(`${suiteId}.schemaVersion: must be 1`);
  if (!Array.isArray(root.cases) || !root.cases.length) {
    throw new Error(`${suiteId}.cases: must be a nonempty array`);
  }
  const cases = root.cases.map((value, index) =>
    parseCase(value, `${suiteId}.cases[${index}]`),
  );
  unique(cases.map((item) => item.id), `${suiteId}.cases`);
  return { cases, domain: suiteId, schemaVersion: 1 };
}

export function scoreFoundation(
  suiteId: string,
  corpus: FoundationCorpus,
  observations: readonly FoundationObservation[],
): FoundationScorecard {
  const failures: string[] = [];
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  if (byId.size !== observations.length) failures.push(`${suiteId}: duplicate observations`);
  let passed = 0;
  const metrics = new Map<string, { cases: number; passed: number }>();
  for (const item of corpus.cases) {
    const observation = byId.get(item.id);
    if (!observation) {
      failures.push(`${suiteId}/${item.id}: missing observation`);
      continue;
    }
    const expected = item.expectation;
    const matchingAssumption = observation.assumptions.some((assumption) =>
      (!expected.assumptionKind || assumption.kind === expected.assumptionKind)
      && (!expected.assumptionValue || assumption.value === expected.assumptionValue)
      && (!expected.assumptionSubject || assumption.subjects.includes(expected.assumptionSubject))
    );
    const mismatches = [
      (expected.assumptionKind || expected.assumptionValue || expected.assumptionSubject)
        && !matchingAssumption
        ? `assumption ${[expected.assumptionKind, expected.assumptionValue, expected.assumptionSubject].filter(Boolean).join("/")}`
        : undefined,
      expected.conceptId && !observation.conceptIds.includes(expected.conceptId)
        ? `concept ${expected.conceptId}`
        : undefined,
      expected.diagnosticCode && !observation.diagnosticCodes.includes(expected.diagnosticCode)
        ? `diagnostic ${expected.diagnosticCode}`
        : undefined,
      expected.excludedConceptId && observation.conceptIds.includes(expected.excludedConceptId)
        ? `excluded concept ${expected.excludedConceptId}`
        : undefined,
      expected.excludedAssumptionValue
        && observation.assumptions.some((item) => item.value === expected.excludedAssumptionValue)
        ? `excluded assumption ${expected.excludedAssumptionValue}`
        : undefined,
      expected.excludedDefinitionSymbol
        && observation.definitions.some((item) => item.symbol === expected.excludedDefinitionSymbol)
        ? `excluded definition ${expected.excludedDefinitionSymbol}`
        : undefined,
      expected.definitionDescription
        && !observation.definitions.some((item) =>
          item.symbol === expected.symbol && item.description === expected.definitionDescription)
        ? `definition ${expected.symbol ?? "?"}/${expected.definitionDescription}`
        : undefined,
      expected.definitionEvidenceRuleId
        && !observation.definitions.some((item) =>
          item.symbol === expected.symbol
          && item.evidenceRuleIds.includes(expected.definitionEvidenceRuleId!))
        ? `definition evidence ${expected.symbol ?? "?"}/${expected.definitionEvidenceRuleId}`
        : undefined,
      expected.excludedQuantityKindId && observation.quantityKindIds.includes(expected.excludedQuantityKindId)
        ? `excluded quantity ${expected.excludedQuantityKindId}`
        : undefined,
      expected.excludedRelationId && observation.relationIds.includes(expected.excludedRelationId)
        ? `excluded relation ${expected.excludedRelationId}`
        : undefined,
      expected.dimension && !observation.dimensions.includes(expected.dimension)
        ? `dimension ${expected.dimension}`
        : undefined,
      expected.quantityKindId && !observation.quantityKindIds.includes(expected.quantityKindId)
        ? `quantity ${expected.quantityKindId}`
        : undefined,
      expected.relationId && !observation.relationIds.includes(expected.relationId)
        ? `relation ${expected.relationId}`
        : undefined,
      expected.status && observation.status !== expected.status
        ? `status ${expected.status}`
        : undefined,
      expected.symbol && !observation.symbols.includes(expected.symbol)
        ? `symbol ${expected.symbol}`
        : undefined,
      expected.unitId && !observation.unitIds.includes(expected.unitId)
        ? `unit ${expected.unitId}`
        : undefined,
    ].filter((value): value is string => Boolean(value));
    const succeeded = mismatches.length === 0;
    if (!succeeded) failures.push(`${suiteId}/${item.id}: missing ${mismatches.join(", ")}`);
    else passed += 1;
    if (item.metric) {
      const metric = metrics.get(item.metric) ?? { cases: 0, passed: 0 };
      metric.cases += 1;
      if (succeeded) metric.passed += 1;
      metrics.set(item.metric, metric);
    }
  }
  for (const observation of observations) {
    if (!corpus.cases.some((item) => item.id === observation.caseId)) {
      failures.push(`${suiteId}/${observation.caseId}: unexpected observation`);
    }
  }
  return {
    cases: corpus.cases.length,
    failures: [...new Set(failures)].sort(),
    metrics: Object.fromEntries([...metrics.entries()].sort(([left], [right]) => left.localeCompare(right))),
    passed,
    schemaVersion: 1,
  };
}

function parseCase(value: unknown, path: string): FoundationCase {
  const item = record(value, path);
  exact(item, ["id", "documents", "cursor", "expectation", "metric", "variationTags"], path);
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
  unique(documents.map((document) => document.fileId), `${path}.documents fileId`);
  const cursorValue = record(item.cursor, `${path}.cursor`);
  exact(cursorValue, ["fileId", "needle", "edge"], `${path}.cursor`);
  const fileId = text(cursorValue.fileId, `${path}.cursor.fileId`);
  const needle = text(cursorValue.needle, `${path}.cursor.needle`);
  const document = documents.find((item) => item.fileId === fileId);
  if (!document) throw new Error(`${path}.cursor.fileId: unknown document ${fileId}`);
  const occurrences = document.content.split(needle).length - 1;
  if (occurrences !== 1) {
    throw new Error(`${path}.cursor.needle: must occur exactly once; found ${occurrences}`);
  }
  const edge = cursorValue.edge === undefined
    ? undefined
    : oneOf(cursorValue.edge, ["before", "after"], `${path}.cursor.edge`);
  const expectationValue = record(item.expectation, `${path}.expectation`);
  exact(
    expectationValue,
    [
      "conceptId",
      "assumptionKind",
      "assumptionSubject",
      "assumptionValue",
      "definitionDescription",
      "definitionEvidenceRuleId",
      "diagnosticCode",
      "dimension",
      "excludedConceptId",
      "excludedAssumptionValue",
      "excludedDefinitionSymbol",
      "excludedQuantityKindId",
      "excludedRelationId",
      "quantityKindId",
      "relationId",
      "status",
      "symbol",
      "unitId",
    ],
    `${path}.expectation`,
  );
  const expectation = Object.fromEntries(
    Object.entries(expectationValue).map(([key, value]) => [
      key,
      text(value, `${path}.expectation.${key}`),
    ]),
  ) as FoundationExpectation;
  if (!Object.keys(expectation).length) throw new Error(`${path}.expectation: must not be empty`);
  if (!Array.isArray(item.variationTags) || !item.variationTags.length) {
    throw new Error(`${path}.variationTags: must be a nonempty array`);
  }
  const variationTags = item.variationTags.map((tag, index) =>
    text(tag, `${path}.variationTags[${index}]`),
  );
  unique(variationTags, `${path}.variationTags`);
  const metric = item.metric === undefined
    ? undefined
    : oneOf(
      item.metric,
      ["association", "assumption", "classification", "evidence", "refusal", "scope"],
      `${path}.metric`,
    );
  return {
    cursor: { ...(edge ? { edge } : {}), fileId, needle },
    documents,
    expectation,
    id: identifier(item.id, `${path}.id`),
    ...(metric ? { metric } : {}),
    variationTags,
  };
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function exact(value: Record<string, unknown>, allowed: readonly string[], path: string): void {
  const unknown = Object.keys(value).filter((key) => !allowed.includes(key));
  if (unknown.length) throw new Error(`${path}: unknown fields: ${unknown.sort().join(", ")}`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${path}: must be nonempty`);
  return value;
}

function identifier(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u.test(result)) {
    throw new Error(`${path}: must be a lowercase kebab-case identifier`);
  }
  return result;
}

function oneOf<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
  path: string,
): Values[number] {
  if (typeof value !== "string" || !values.includes(value)) {
    throw new Error(`${path}: must be one of ${values.join(", ")}`);
  }
  return value as Values[number];
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: duplicate values`);
}
