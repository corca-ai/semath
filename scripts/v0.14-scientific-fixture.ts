import type {
  QueryResult,
  SemanticContextInfo,
  SemathQuery,
} from "../packages/protocol/src/index";

interface ScientificDocument {
  content: string;
  fileId: string;
  language: "latex" | "markdown";
  path: string;
}

export interface ScientificTarget {
  expectedConcept?: string;
  expectedDerivedFrom?: readonly string[];
  expectedDiagnosticCodes?: readonly string[];
  expectedDimension?: string;
  expectedPatterns?: readonly string[];
  expectedQuantityKind?: string;
  expectedRelation?: string;
  expectedUnit?: string;
  fileId: string;
  id: string;
  kind: SemathQuery["kind"];
  needle: string;
  needleOffset?: number;
}

export interface ScientificCorpus {
  documents: readonly ScientificDocument[];
  targets: readonly ScientificTarget[];
}

export function buildScientificFixture(corpus: ScientificCorpus) {
  const epoch = "scientific:v0.14";
  const documents = corpus.documents.map((document) => ({
    ...document,
    documentVersion: 1,
  }));
  const byId = new Map(documents.map((document) => [document.fileId, document]));
  const queries = corpus.targets.map((target) => {
    const document = byId.get(target.fileId);
    if (!document) throw new Error(`${target.id}: missing document ${target.fileId}`);
    const start = document.content.indexOf(target.needle);
    if (start < 0 || document.content.indexOf(target.needle, start + 1) >= 0) {
      throw new Error(`${target.id}: needle must occur exactly once`);
    }
    const offset =
      start +
      (target.needleOffset ?? Math.max(1, Math.floor(target.needle.length / 2)));
    const query =
      target.kind === "diagnostics"
        ? { fileId: target.fileId, kind: target.kind }
        : { fileId: target.fileId, kind: target.kind, offset };
    return {
      protocolVersion: 1 as const,
      epoch,
      inventoryVersion: 1,
      documentVersion: 1,
      analysisGeneration: 1,
      query,
    };
  });
  return {
    expectations: corpus.targets,
    fixture: {
      snapshot: {
        protocolVersion: 1 as const,
        epoch,
        inventoryVersion: 1,
        projectId: "v0.14-scientific-foundation",
        mainFileId: null,
        documents,
      },
      queries,
    },
  };
}

export function assertScientificResults(
  results: readonly QueryResult[],
  expectations: readonly ScientificTarget[],
) {
  if (results.length !== expectations.length) {
    throw new Error(`expected ${expectations.length} results, got ${results.length}`);
  }
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (!value) throw new Error(`${expectation.id}: missing result`);
    if (expectation.expectedPatterns) {
      const recognitions =
        value.kind === "formulaRecognitions"
          ? value.recognitions
          : value.kind === "inspection"
            ? value.inspection.recognitions
            : undefined;
      if (!recognitions) throw new Error(`${expectation.id}: missing recognitions`);
      assertSame(
        expectation.id,
        recognitions.map((recognition) => recognition.patternId),
        expectation.expectedPatterns,
      );
    }
    if (expectation.expectedDiagnosticCodes) {
      if (value.kind !== "diagnostics") {
        throw new Error(`${expectation.id}: missing diagnostics`);
      }
      assertSame(
        expectation.id,
        value.diagnostics.map((diagnostic) => diagnostic.code),
        expectation.expectedDiagnosticCodes,
      );
    }
    const context = semanticContext(value);
    if (expectation.expectedRelation) {
      if (!context?.relations.some(
        (relation) => relation.relationId === expectation.expectedRelation,
      )) {
        throw new Error(`${expectation.id}: missing relation ${expectation.expectedRelation}`);
      }
    }
    if (expectation.expectedConcept) {
      if (!context?.concepts.some(
        (concept) => concept.conceptId === expectation.expectedConcept,
      )) {
        throw new Error(`${expectation.id}: missing concept ${expectation.expectedConcept}`);
      }
    }
    const quantity = context?.quantities[0];
    if (
      expectation.expectedQuantityKind &&
      quantity?.quantityKindId !== expectation.expectedQuantityKind
    ) {
      throw new Error(`${expectation.id}: unexpected quantity kind`);
    }
    if (expectation.expectedUnit && quantity?.unitId !== expectation.expectedUnit) {
      throw new Error(`${expectation.id}: unexpected unit`);
    }
    if (
      expectation.expectedDimension &&
      quantity?.dimension.display !== expectation.expectedDimension
    ) {
      throw new Error(`${expectation.id}: unexpected dimension ${quantity?.dimension.display}`);
    }
    if (expectation.expectedDerivedFrom) {
      assertSame(
        expectation.id,
        quantity?.derivedFrom ?? [],
        expectation.expectedDerivedFrom,
      );
    }
  }
  return { queries: results.length };
}

function semanticContext(
  value: QueryResult["value"],
): SemanticContextInfo | undefined {
  if (value.kind === "semanticContext") return value.context;
  if (value.kind === "inspection") return value.inspection.semantic;
  return undefined;
}

function assertSame(
  id: string,
  actual: readonly string[],
  expected: readonly string[],
) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  if (left.length !== right.length || left.some((value, index) => value !== right[index])) {
    throw new Error(`${id}: expected [${right.join(", ")}], got [${left.join(", ")}]`);
  }
}
