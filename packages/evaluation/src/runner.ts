import { LatexSyntaxService } from "wasmtex/syntax";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type MathFormulaAnchorInfo,
  type MathInterpretationHypothesisInfo,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
} from "../../protocol/src/index";
import {
  adaptNonLatexDocument,
  adaptWasmtexDocument,
} from "../../wasmtex-adapter/src/index";
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
import { sameFormulaAnchor } from "./challenge-observation";
import type { CaseObservation } from "./scorecard";

type QualityFormulaHypothesis = Pick<
  MathInterpretationHypothesisInfo,
  "conditions" | "formula" | "kind" | "relation" | "support"
>;

interface QualityFormulaContext {
  readonly formula?: MathFormulaAnchorInfo;
  readonly interpretations: {
    readonly hypotheses: readonly QualityFormulaHypothesis[];
  };
}

type ObservedQualityRelation = Omit<
  NonNullable<MathInterpretationHypothesisInfo["relation"]>,
  "conditions"
> & {
  conditions: MathInterpretationHypothesisInfo["conditions"];
};

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
        documents.push(adaptNonLatexDocument({
          content: input.content,
          documentVersion: 1,
          fileId: input.fileId,
          language,
          path: input.path,
        }));
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
  const observedRelations = observedQualityRelations(view?.authoringContext).map(
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
    if (item.case.expectation === "recognized" && targetPresent) {
      reason = "The target law matched with the required typed evidence.";
    } else if (item.case.expectation === "refused" && !targetPresent) {
      reason = `The target law was safely refused${
        "refusalCategory" in item.case ? ` (${item.case.refusalCategory})` : ""
      }.`;
    } else if (view.decision.status === "conflicting") {
      reason = view.decision.conflicts.map((conflict) => conflict.label).join("; ");
    } else if (view.decision.status === "partial") {
      reason = [...view.decision.requirements, ...view.decision.reasons]
        .map((item) => item.label)
        .join("; ");
    } else if (view.decision.status === "unsupported") {
      reason = view.decision.reasons.map((item) => item.label).join("; ");
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
    status: view?.authoringContext.disposition ?? "missing",
    suiteId: item.suiteId,
  };
}

function observe(
  item: PlannedQualityCase,
  result: QueryResult | undefined,
): CaseObservation {
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const targetLawId = "lawId" in item.case ? item.case.lawId : undefined;
  const relations = observedQualityRelations(view?.authoringContext);
  const targetRelations = targetLawId
    ? relations.filter((candidate) =>
        candidate.relationId.endsWith(`:${targetLawId}`),
      )
    : [];
  const expectedRoles =
    "expectedRoles" in item.case ? item.case.expectedRoles : undefined;
  const relation =
    targetRelations.find((candidate) =>
      rolesMatch(candidate.roles, expectedRoles, item.case.macros),
    ) ?? targetRelations[0];
  const recognizedLawIds = [
    ...new Set(
      relations.map((candidate) =>
        candidate.relationId.slice(candidate.relationId.lastIndexOf(":") + 1),
      ),
    ),
  ].sort();
  return {
    caseId: item.case.id,
    evidenceIntegrity: targetRelations.some(
      (candidate) =>
        rolesMatch(candidate.roles, expectedRoles, item.case.macros) &&
        evidenceIsSourceLinked(candidate.evidence, candidate.conditions),
    ),
    recognizedLawIds,
    ...(item.generatedFrom ? { generatedFrom: item.generatedFrom } : {}),
    rolesCorrect: rolesMatch(
      relation?.roles ?? [],
      expectedRoles,
      item.case.macros,
    ),
    ...(view ? { status: view.authoringContext.disposition } : {}),
    suiteId: item.suiteId,
    targetPresent: targetRelations.length > 0,
  };
}

export function observedQualityRelations(
  context: QualityFormulaContext | undefined,
): readonly ObservedQualityRelation[] {
  const selected = context?.formula;
  if (!selected) return [];
  const relations = context.interpretations.hypotheses
    .filter(
      (hypothesis) =>
        hypothesis.kind === "typed-law" &&
        hypothesis.relation !== undefined &&
        hypothesis.formula !== undefined &&
        hypothesis.support !== "contradicted" &&
        sameFormulaAnchor(hypothesis.formula, selected),
    )
    .map((hypothesis) => ({
      ...hypothesis.relation!,
      conditions: hypothesis.conditions,
    }));
  return relations
    .sort((left, right) => qualityRelationKey(left).localeCompare(qualityRelationKey(right)))
    .filter(
      (relation, index, all) =>
        index === 0 || qualityRelationKey(all[index - 1]!) !== qualityRelationKey(relation),
    );
}

function qualityRelationKey(relation: ObservedQualityRelation): string {
  const roles = [...relation.roles]
    .map((role) => `${role.role}\u0000${role.symbol}\u0000${role.conceptId ?? ""}`)
    .sort();
  const evidence = relation.evidence
    .flatMap((item) => item.sourceRanges)
    .map((range) => `${range.startOffset}:${range.endOffset}`)
    .sort();
  const conditions = [...relation.conditions]
    .map((condition) => `${condition.conditionId}\u0000${condition.status}`)
    .sort();
  return [relation.relationId, ...roles, "evidence", ...evidence, "conditions", ...conditions].join(
    "\u0001",
  );
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
