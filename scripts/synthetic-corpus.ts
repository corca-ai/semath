export type SyntheticPurpose = "recognition" | "refusal" | "coverage";

export interface SyntheticFormulaCase {
  id: string;
  topic: string;
  purpose: SyntheticPurpose;
  language: "latex" | "markdown";
  source: string;
  cursorNeedle: string;
  expectedPatterns: string[];
}

export interface SyntheticDomainCorpus {
  schemaVersion: 1;
  domain: string;
  cases: SyntheticFormulaCase[];
}

interface FormulaRecognitionResult {
  value?: {
    kind?: string;
    recognitions?: Array<{ patternId?: string }>;
  };
}

export interface SyntheticExpectation {
  case: SyntheticFormulaCase;
  domain: string;
}

export interface SyntheticScorecard {
  domain: string;
  cases: number;
  recognition: number;
  refusals: number;
  coverageTargets: number;
  supportedCoverageTargets: number;
  semanticCoveragePercent: number;
}

import type { SemanticQualityObservation } from "./semantic-quality";

export function parseSyntheticDomainCorpus(
  value: unknown,
  sourceName: string,
): SyntheticDomainCorpus {
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
    const purpose = item.purpose;
    if (
      purpose !== "recognition" &&
      purpose !== "refusal" &&
      purpose !== "coverage"
    ) {
      throw new Error(`${path}.purpose is invalid`);
    }
    const language = item.language;
    if (language !== "latex" && language !== "markdown") {
      throw new Error(`${path}.language is invalid`);
    }
    const source = nonEmptyString(item.source, `${path}.source`);
    const cursorNeedle = nonEmptyString(
      item.cursorNeedle,
      `${path}.cursorNeedle`,
    );
    const firstNeedle = source.indexOf(cursorNeedle);
    if (firstNeedle < 0 || source.indexOf(cursorNeedle, firstNeedle + 1) >= 0) {
      throw new Error(`${path}.cursorNeedle must occur exactly once in source`);
    }
    if (
      !Array.isArray(item.expectedPatterns) ||
      item.expectedPatterns.some(
        (pattern) => typeof pattern !== "string" || pattern.length === 0,
      )
    ) {
      throw new Error(`${path}.expectedPatterns must contain strings`);
    }
    const expectedPatterns = [...new Set(item.expectedPatterns as string[])];
    if (purpose === "recognition" && expectedPatterns.length === 0) {
      throw new Error(`${path}: recognition requires an expected pattern`);
    }
    // A domain-specific refusal may still be recognized by a more general pack.
    // Exact result comparison below guarantees that the domain pattern remains absent.
    return {
      id,
      topic: nonEmptyString(item.topic, `${path}.topic`),
      purpose,
      language,
      source,
      cursorNeedle,
      expectedPatterns,
    };
  });
  return { schemaVersion: 1, domain, cases };
}

export function buildSyntheticFormulaFixture(corpora: SyntheticDomainCorpus[]) {
  if (corpora.length === 0) throw new Error("synthetic corpus is empty");
  const epoch = "synthetic:v1";
  const expectations: SyntheticExpectation[] = [];
  const seenIds = new Set<string>();
  const seenSources = new Set<string>();
  const documents = [];
  const queries = [];

  for (const corpus of [...corpora].sort((left, right) =>
    left.domain.localeCompare(right.domain),
  )) {
    for (const [index, entry] of corpus.cases.entries()) {
      const qualifiedId = `${corpus.domain}/${entry.id}`;
      if (seenIds.has(qualifiedId)) {
        throw new Error(`duplicate synthetic case ${qualifiedId}`);
      }
      if (seenSources.has(entry.source)) {
        throw new Error(`${qualifiedId}: duplicate source across corpus`);
      }
      seenIds.add(qualifiedId);
      seenSources.add(entry.source);
      const fileId = `synthetic-${corpus.domain}-${index}`;
      const needleStart = entry.source.indexOf(entry.cursorNeedle);
      const offset = needleStart + Math.floor(entry.cursorNeedle.length / 2);
      documents.push({
        fileId,
        path: `synthetic/${corpus.domain}/${entry.id}.${entry.language === "latex" ? "tex" : "md"}`,
        language: entry.language,
        content: entry.source,
        documentVersion: 1,
      });
      queries.push({
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        documentVersion: 1,
        analysisGeneration: 1,
        query: { kind: "formulaRecognition", fileId, offset },
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
        projectId: "synthetic-v1",
        mainFileId: documents[0]?.fileId ?? null,
        documents,
      },
      queries,
    },
  };
}

export function assertSyntheticFormulaResults(
  results: FormulaRecognitionResult[],
  expectations: SyntheticExpectation[],
): SyntheticScorecard[] {
  if (results.length !== expectations.length) {
    throw new Error(
      `synthetic result count ${results.length} differs from ${expectations.length}`,
    );
  }
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (value?.kind !== "formulaRecognitions" || !Array.isArray(value.recognitions)) {
      throw new Error(`${expectation.domain}/${expectation.case.id}: missing recognition result`);
    }
    const actual = value.recognitions.flatMap((recognition) =>
      recognition.patternId ? [recognition.patternId] : [],
    );
    if (!sameValues(actual, expectation.case.expectedPatterns)) {
      throw new Error(
        `${expectation.domain}/${expectation.case.id}: expected [${expectation.case.expectedPatterns.join(", ")}], got [${actual.join(", ")}]`,
      );
    }
  }
  return scoreSyntheticCorpora(expectations);
}

export function observeSyntheticFormulaResults(
  results: FormulaRecognitionResult[],
  expectations: SyntheticExpectation[],
): SemanticQualityObservation[] {
  if (results.length !== expectations.length) {
    throw new Error(
      `synthetic result count ${results.length} differs from ${expectations.length}`,
    );
  }
  return expectations.map((expectation, index) => {
    const value = results[index]?.value;
    if (value?.kind !== "formulaRecognitions" || !Array.isArray(value.recognitions)) {
      throw new Error(`${expectation.domain}/${expectation.case.id}: missing recognition result`);
    }
    const actual = value.recognitions.flatMap((recognition) =>
      recognition.patternId ? [recognition.patternId] : [],
    );
    const expected = expectation.case.expectedPatterns;
    const expectedSet = new Set(expected);
    const matchedItems = actual.filter((pattern) => expectedSet.has(pattern)).length;
    return {
      field: "formula",
      domain: expectation.domain,
      topic: expectation.case.topic,
      capability: formulaCapability(expectation.case),
      cases: 1,
      exactCases: sameValues(actual, expected) ? 1 : 0,
      expectedItems: expected.length,
      matchedItems,
      actualItems: actual.length,
      unexpectedItems: actual.length - matchedItems,
    };
  });
}

export function scoreSyntheticCorpora(
  expectations: SyntheticExpectation[],
): SyntheticScorecard[] {
  const domains = new Map<string, SyntheticExpectation[]>();
  for (const expectation of expectations) {
    const entries = domains.get(expectation.domain) ?? [];
    entries.push(expectation);
    domains.set(expectation.domain, entries);
  }
  return [...domains]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([domain, entries]) => {
      const recognition = countPurpose(entries, "recognition");
      const refusals = countPurpose(entries, "refusal");
      const coverage = entries.filter(
        (entry) => entry.case.purpose === "coverage",
      );
      const supportedCoverageTargets = coverage.filter(
        (entry) => entry.case.expectedPatterns.length > 0,
      ).length;
      const semanticTargets = recognition + coverage.length;
      return {
        domain,
        cases: entries.length,
        recognition,
        refusals,
        coverageTargets: coverage.length,
        supportedCoverageTargets,
        semanticCoveragePercent:
          semanticTargets === 0
            ? 100
            : Math.round(
                ((recognition + supportedCoverageTargets) / semanticTargets) *
                  1000,
              ) / 10,
      };
    });
}

function countPurpose(
  entries: SyntheticExpectation[],
  purpose: SyntheticPurpose,
) {
  return entries.filter((entry) => entry.case.purpose === purpose).length;
}

function formulaCapability(entry: SyntheticFormulaCase) {
  if (entry.purpose !== "coverage") return entry.purpose;
  return entry.expectedPatterns.length > 0
    ? "supported-coverage"
    : "coverage-holdout";
}

function sameValues(actual: string[], expected: string[]) {
  const sortedActual = [...actual].sort();
  const sortedExpected = [...expected].sort();
  return (
    sortedActual.length === sortedExpected.length &&
    sortedActual.every((value, index) => value === sortedExpected[index])
  );
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
