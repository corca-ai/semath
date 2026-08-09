import { LatexSyntaxService } from "wasmtex/syntax";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
} from "../../protocol/src/index";
import { adaptWasmtexDocument } from "../../wasmtex-adapter/src/index";
import type {
  Corpus,
  CorpusCase,
  CorpusDocument,
  MetamorphicTransform,
  QualityManifest,
} from "./model";
import { planMetamorphicCases } from "./metamorphic";
import {
  evidenceIsSourceLinked,
  rolesMatch,
} from "./observation";
import type { CaseObservation } from "./scorecard";

export interface PlannedQualityCase {
  case: CorpusCase;
  generatedFrom?: {
    caseId: string;
    transform: MetamorphicTransform;
  };
  suiteId: string;
}

export interface QualityRunPlan {
  planned: readonly PlannedQualityCase[];
  queries: readonly QueryEnvelope[];
  snapshot: ProjectSnapshot;
}

export interface CaseExplanation {
  caseId: string;
  diagnosticCodes: readonly string[];
  expected: string;
  observedRelations: readonly string[];
  reason: string;
  status: string;
  suiteId: string;
}

export function planQualityRun(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
): QualityRunPlan {
  const planned: PlannedQualityCase[] = manifest.suites.flatMap((suite) => {
    const corpus = corpora.get(suite.id);
    if (!corpus) throw new Error(`${suite.id}: corpus was not loaded`);
    return [...corpus.cases]
      .sort((left, right) => left.id.localeCompare(right.id))
      .map((item) => ({ case: item, suiteId: suite.id }));
  });
  planned.push(
    ...planMetamorphicCases(manifest, corpora).map((item) => ({
      case: item.case,
      generatedFrom: {
        caseId: item.sourceCaseId,
        transform: item.transform,
      },
      suiteId: item.suiteId,
    })),
  );

  const documents: ProjectDocument[] = [];
  const queries: QueryEnvelope[] = [];
  for (const item of planned) {
    const prefix = `${item.suiteId}/${item.case.id}/`;
    const inputs = materializeDocuments(item.case, prefix);
    const syntax = new LatexSyntaxService();
    syntax.reset({
      documents: inputs
        .filter((document) => languageOf(document.path) !== "bibtex")
        .map((document) => ({ ...document, documentVersion: 1 })),
    });
    for (const input of inputs) {
      const language = languageOf(input.path);
      if (language === "bibtex") {
        documents.push({
          ...input,
          documentVersion: 1,
          includes: [],
          language,
          macros: [],
          mathRegions: [],
        });
        continue;
      }
      const snapshot = syntax.getFile(input.fileId);
      if (!snapshot) throw new Error(`${item.suiteId}/${item.case.id}: missing syntax`);
      documents.push(
        adaptWasmtexDocument({ content: input.content, language, syntax: snapshot }),
      );
    }
    const cursorDocument = inputs.find(
      (document) => document.fileId === prefix + item.case.cursor.fileId,
    );
    if (!cursorDocument) {
      throw new Error(`${item.suiteId}/${item.case.id}: unknown cursor file`);
    }
    const first = cursorDocument.content.indexOf(item.case.cursor.needle);
    const last = cursorDocument.content.lastIndexOf(item.case.cursor.needle);
    if (first < 0 || first !== last) {
      throw new Error(
        `${item.suiteId}/${item.case.id}: cursor needle must occur exactly once`,
      );
    }
    queries.push({
      analysisGeneration: 0,
      documentVersion: 1,
      epoch: "quality-corpus",
      inventoryVersion: 1,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query: {
        fileId: prefix + item.case.cursor.fileId,
        kind: "semanticView",
        offset:
          item.case.cursor.edge === "after"
            ? first + item.case.cursor.needle.length
            : first,
      },
    });
  }
  return {
    planned,
    queries,
    snapshot: {
      documents,
      epoch: "quality-corpus",
      inventoryVersion: 1,
      projectId: "quality-corpus",
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    },
  };
}

export function observeQualityRun(
  plan: QualityRunPlan,
  results: readonly QueryResult[],
): CaseObservation[] {
  if (results.length !== plan.planned.length) {
    throw new Error(
      `quality run returned ${results.length}/${plan.planned.length} results`,
    );
  }
  return plan.planned.map((item, index) => observe(item, results[index]));
}

export function explainQualityCase(
  item: PlannedQualityCase,
  result: QueryResult | undefined,
): CaseExplanation {
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const observedRelations = (view?.context.relations ?? []).map(
    (relation) => relation.relationId,
  );
  const target = "lawId" in item.case ? item.case.lawId : undefined;
  const targetPresent = target
    ? observedRelations.some((relation) => relation.endsWith(`:${target}`))
    : false;
  const expected = target
    ? `${item.case.expectation}:${target}`
    : item.case.expectation;
  let reason = "No semantic view was returned.";
  if (view) {
    if (item.case.expectation === "established" && targetPresent) {
      reason = "The target law matched with the required typed evidence.";
    } else if (item.case.expectation === "refused" && !targetPresent) {
      reason = `The target law was safely refused${
        "refusalCategory" in item.case ? ` (${item.case.refusalCategory})` : ""
      }.`;
    } else if (view.refusal) {
      reason = view.refusal;
    } else if (view.diagnostics.length) {
      reason = view.diagnostics.map((diagnostic) => diagnostic.message).join("; ");
    } else {
      reason = targetPresent
        ? "The target law matched although the case expected refusal."
        : "The target law did not acquire enough compatible role and constraint evidence.";
    }
  }
  return {
    caseId: item.case.id,
    diagnosticCodes: (view?.diagnostics ?? []).map((diagnostic) => diagnostic.code),
    expected,
    observedRelations,
    reason,
    status: view?.status ?? "missing",
    suiteId: item.suiteId,
  };
}

function observe(
  item: PlannedQualityCase,
  result: QueryResult | undefined,
): CaseObservation {
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const targetLawId = "lawId" in item.case ? item.case.lawId : undefined;
  const relation = targetLawId
    ? view?.context.relations.find((candidate) =>
        candidate.relationId.endsWith(`:${targetLawId}`),
      )
    : undefined;
  const establishedLawIds = [
    ...new Set(
      (view?.context.relations ?? []).map((candidate) =>
        candidate.relationId.slice(candidate.relationId.lastIndexOf(":") + 1),
      ),
    ),
  ].sort();
  return {
    caseId: item.case.id,
    evidenceIntegrity: Boolean(
      relation && evidenceIsSourceLinked(relation.evidence, relation.conditions),
    ),
    establishedLawIds,
    ...(item.generatedFrom ? { generatedFrom: item.generatedFrom } : {}),
    rolesCorrect: rolesMatch(
      relation?.roles ?? [],
      "expectedRoles" in item.case ? item.case.expectedRoles : undefined,
      item.case.macros,
    ),
    ...(view ? { status: view.status } : {}),
    suiteId: item.suiteId,
    targetPresent: Boolean(relation),
  };
}

function materializeDocuments(entry: CorpusCase, prefix: string): CorpusDocument[] {
  const inputs = entry.documents.map((document) => ({
    ...document,
    fileId: prefix + document.fileId,
    path: prefix + document.path,
  }));
  const missing = (entry.macros ?? []).filter(
    (macro) =>
      !inputs.some((document) =>
        document.content.includes(`\\newcommand{${macro.name}}`),
      ),
  );
  if (missing.length) {
    const main = inputs.find(
      (document) => document.fileId === prefix + entry.cursor.fileId,
    );
    if (!main) throw new Error(`${entry.id}: macro case has no cursor document`);
    const preamble = missing
      .map(
        (macro) =>
          `\\newcommand{${macro.name}}${macro.parameterCount ? `[${macro.parameterCount}]` : ""}{${macro.definition}}`,
      )
      .join("\n");
    main.content = `${preamble}\n${main.content}`;
  }
  return inputs;
}

function languageOf(path: string): DocumentLanguage {
  if (/\.md$/iu.test(path)) return "markdown";
  if (/\.bib$/iu.test(path)) return "bibtex";
  return "latex";
}
