import type { SyntheticPurpose } from "./synthetic-corpus";
import type { SemanticQualityObservation } from "./semantic-quality";

const CURSOR_MARKER = "<<CURSOR>>";

export interface ExpectedSyntheticDefinition {
  description: string;
  ruleId: string;
}

export interface SyntheticProseCase {
  id: string;
  topic: string;
  purpose: SyntheticPurpose;
  language: "latex" | "markdown";
  annotatedSource: string;
  expectedDefinitions: ExpectedSyntheticDefinition[];
}

export interface SyntheticProseCorpus {
  schemaVersion: 1;
  domain: string;
  cases: SyntheticProseCase[];
}

export interface SyntheticProseExpectation {
  case: SyntheticProseCase;
  domain: string;
}

interface HoverResult {
  value?: {
    kind?: string;
    definitions?: Array<{
      description?: string;
      evidence?: { ruleId?: string };
    }>;
  };
}

export function parseSyntheticProseCorpus(
  value: unknown,
  sourceName: string,
): SyntheticProseCorpus {
  const root = record(value, sourceName);
  if (root.schemaVersion !== 1) {
    throw new Error(`${sourceName}: schemaVersion must be 1`);
  }
  const domain = nonEmptyString(root.domain, `${sourceName}.domain`);
  if (!Array.isArray(root.cases) || root.cases.length === 0) {
    throw new Error(`${sourceName}.cases must be a non-empty array`);
  }
  const ids = new Set<string>();
  const cases = root.cases.map((entry, index) => {
    const path = `${sourceName}.cases[${index}]`;
    const item = record(entry, path);
    const id = nonEmptyString(item.id, `${path}.id`);
    if (ids.has(id)) throw new Error(`${path}.id duplicates ${id}`);
    ids.add(id);
    const purpose = parsePurpose(item.purpose, `${path}.purpose`);
    const language = item.language;
    if (language !== "latex" && language !== "markdown") {
      throw new Error(`${path}.language is invalid`);
    }
    const annotatedSource = nonEmptyString(
      item.annotatedSource,
      `${path}.annotatedSource`,
    );
    const marker = annotatedSource.indexOf(CURSOR_MARKER);
    if (
      marker < 0 ||
      annotatedSource.indexOf(CURSOR_MARKER, marker + CURSOR_MARKER.length) >= 0
    ) {
      throw new Error(`${path}.annotatedSource must contain one cursor marker`);
    }
    if (!Array.isArray(item.expectedDefinitions)) {
      throw new Error(`${path}.expectedDefinitions must be an array`);
    }
    const expectedDefinitions = item.expectedDefinitions.map(
      (definition, definitionIndex) => {
        const definitionPath = `${path}.expectedDefinitions[${definitionIndex}]`;
        const expected = record(definition, definitionPath);
        return {
          description: nonEmptyString(
            expected.description,
            `${definitionPath}.description`,
          ),
          ruleId: nonEmptyString(expected.ruleId, `${definitionPath}.ruleId`),
        };
      },
    );
    if (purpose === "recognition" && expectedDefinitions.length === 0) {
      throw new Error(`${path}: recognition requires a definition`);
    }
    if (purpose === "refusal" && expectedDefinitions.length !== 0) {
      throw new Error(`${path}: refusal cannot expect a definition`);
    }
    return {
      id,
      topic: nonEmptyString(item.topic, `${path}.topic`),
      purpose,
      language,
      annotatedSource,
      expectedDefinitions,
    };
  });
  return { schemaVersion: 1, domain, cases };
}

export function buildSyntheticProseFixture(corpora: SyntheticProseCorpus[]) {
  if (corpora.length === 0) throw new Error("synthetic prose corpus is empty");
  const epoch = "synthetic:v1-prose";
  const documents = [];
  const queries = [];
  const expectations: SyntheticProseExpectation[] = [];
  const sources = new Set<string>();

  for (const corpus of [...corpora].sort((left, right) =>
    left.domain.localeCompare(right.domain),
  )) {
    for (const [index, entry] of corpus.cases.entries()) {
      const offset = entry.annotatedSource.indexOf(CURSOR_MARKER);
      const content = entry.annotatedSource.replace(CURSOR_MARKER, "");
      if (sources.has(content)) {
        throw new Error(`${corpus.domain}/${entry.id}: duplicate source`);
      }
      sources.add(content);
      const fileId = `synthetic-prose-${corpus.domain}-${index}`;
      documents.push({
        fileId,
        path: `synthetic/prose/${entry.id}.${entry.language === "latex" ? "tex" : "md"}`,
        language: entry.language,
        content,
        documentVersion: 1,
      });
      queries.push({
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        documentVersion: 1,
        analysisGeneration: 1,
        query: { kind: "hover", fileId, offset },
      });
      expectations.push({ case: entry, domain: corpus.domain });
    }
  }
  return {
    expectations,
    fixture: {
      snapshot: {
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        projectId: "synthetic-v1-prose",
        mainFileId: documents[0]?.fileId ?? null,
        documents,
      },
      queries,
    },
  };
}

export function assertSyntheticProseResults(
  results: HoverResult[],
  expectations: SyntheticProseExpectation[],
) {
  if (results.length !== expectations.length) {
    throw new Error(
      `synthetic prose result count ${results.length} differs from ${expectations.length}`,
    );
  }
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (value?.kind !== "hover" || !Array.isArray(value.definitions)) {
      throw new Error(`${expectation.domain}/${expectation.case.id}: missing hover result`);
    }
    const actual = value.definitions.map((definition) => ({
      description: definition.description ?? "",
      ruleId: definition.evidence?.ruleId ?? "",
    }));
    if (!sameDefinitions(actual, expectation.case.expectedDefinitions)) {
      throw new Error(
        `${expectation.domain}/${expectation.case.id}: expected ${formatDefinitions(expectation.case.expectedDefinitions)}, got ${formatDefinitions(actual)}`,
      );
    }
  }
  const recognition = expectations.filter(
    (entry) => entry.case.purpose === "recognition",
  ).length;
  const refusals = expectations.filter(
    (entry) => entry.case.purpose === "refusal",
  ).length;
  const coverage = expectations.filter(
    (entry) => entry.case.purpose === "coverage",
  );
  const supportedCoverageTargets = coverage.filter(
    (entry) => entry.case.expectedDefinitions.length > 0,
  ).length;
  return {
    cases: expectations.length,
    recognition,
    refusals,
    coverageTargets: coverage.length,
    supportedCoverageTargets,
    semanticCoveragePercent:
      recognition + coverage.length === 0
        ? 100
        : Math.round(
            ((recognition + supportedCoverageTargets) /
              (recognition + coverage.length)) *
              1000,
          ) / 10,
  };
}

export function observeSyntheticProseResults(
  results: HoverResult[],
  expectations: SyntheticProseExpectation[],
): SemanticQualityObservation[] {
  if (results.length !== expectations.length) {
    throw new Error(
      `synthetic prose result count ${results.length} differs from ${expectations.length}`,
    );
  }
  return expectations.map((expectation, index) => {
    const value = results[index]?.value;
    if (value?.kind !== "hover" || !Array.isArray(value.definitions)) {
      throw new Error(`${expectation.domain}/${expectation.case.id}: missing hover result`);
    }
    const actual = value.definitions.map((definition) => ({
      description: definition.description ?? "",
      ruleId: definition.evidence?.ruleId ?? "",
    }));
    const expected = expectation.case.expectedDefinitions;
    const expectedKeys = new Set(expected.map(definitionKey));
    const matchedItems = actual.filter((definition) =>
      expectedKeys.has(definitionKey(definition)),
    ).length;
    return {
      field: "prose",
      domain: expectation.domain,
      topic: expectation.case.topic,
      capability:
        expectation.case.purpose === "coverage"
          ? expected.length > 0
            ? "supported-coverage"
            : "coverage-holdout"
          : expectation.case.purpose,
      cases: 1,
      exactCases: sameDefinitions(actual, expected) ? 1 : 0,
      expectedItems: expected.length,
      matchedItems,
      actualItems: actual.length,
      unexpectedItems: actual.length - matchedItems,
    };
  });
}

function sameDefinitions(
  actual: ExpectedSyntheticDefinition[],
  expected: ExpectedSyntheticDefinition[],
) {
  const left = actual.map(definitionKey).sort();
  const right = expected.map(definitionKey).sort();
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function definitionKey(definition: ExpectedSyntheticDefinition) {
  return `${definition.ruleId}\0${definition.description}`;
}

function formatDefinitions(definitions: ExpectedSyntheticDefinition[]) {
  return `[${definitions
    .map((definition) => `${definition.ruleId}:${definition.description}`)
    .join(", ")}]`;
}

function parsePurpose(value: unknown, path: string): SyntheticPurpose {
  if (value !== "recognition" && value !== "refusal" && value !== "coverage") {
    throw new Error(`${path} is invalid`);
  }
  return value;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function nonEmptyString(value: unknown, path: string) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${path} must be a non-empty string`);
  }
  return value;
}
