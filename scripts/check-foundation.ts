import { spawnSync } from "node:child_process";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  type FoundationObservation,
  scoreFoundation,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import {
  loadFoundationFixtures,
  loadQualityFixtures,
} from "./evaluation-fixtures";

const { manifest } = await loadQualityFixtures();
const corpora = await loadFoundationFixtures(manifest);
const planned = manifest.foundationSuites.flatMap((suite) => {
  const corpus = corpora.get(suite.id);
  if (!corpus) throw new Error(`${suite.id}: foundation corpus was not loaded`);
  return corpus.cases.map((item) => ({ case: item, suite }));
});
const documents: ProjectDocument[] = [];
const queries: QueryEnvelope[] = [];
for (const item of planned) {
  const prefix = `${item.suite.id}/${item.case.id}/`;
  const inputs = item.case.documents.map((document) => ({
    ...document,
    fileId: prefix + document.fileId,
    path: prefix + document.path,
  }));
  const syntax = new LatexSyntaxService();
  syntax.reset({
    documents: inputs.map((document) => ({ ...document, documentVersion: 1 })),
  });
  for (const input of inputs) {
    const snapshot = syntax.getFile(input.fileId);
    if (!snapshot) throw new Error(`${item.suite.id}/${item.case.id}: missing syntax`);
    documents.push(
      adaptWasmtexDocument({
        content: input.content,
        language: languageOf(input.path),
        syntax: snapshot,
      }),
    );
  }
  const cursorDocument = inputs.find(
    (document) => document.fileId === prefix + item.case.cursor.fileId,
  );
  if (!cursorDocument) throw new Error(`${item.case.id}: unknown cursor document`);
  const first = cursorDocument.content.indexOf(item.case.cursor.needle);
  const last = cursorDocument.content.lastIndexOf(item.case.cursor.needle);
  if (first < 0 || first !== last) throw new Error(`${item.case.id}: ambiguous cursor needle`);
  queries.push({
    analysisGeneration: 0,
    documentVersion: 1,
    epoch: "foundation-corpus",
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: {
      fileId: prefix + item.case.cursor.fileId,
      kind: "semanticView",
      offset: item.case.cursor.edge === "after"
        ? first + item.case.cursor.needle.length
        : first,
    },
  });
}

const native = spawnSync(
  "cargo",
  ["run", "--quiet", "--locked", "-p", "semath-native"],
  {
    encoding: "utf8",
    input: JSON.stringify({
      queries,
      snapshot: {
        documents,
        epoch: "foundation-corpus",
        inventoryVersion: 1,
        projectId: "foundation-corpus",
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      },
    }),
    maxBuffer: 64 * 1024 * 1024,
  },
);
if (native.status !== 0) throw new Error(native.stderr || "native foundation run failed");
const results = JSON.parse(native.stdout) as QueryResult[];
if (results.length !== planned.length) {
  throw new Error(`native foundation run returned ${results.length}/${planned.length}`);
}

const observations = planned.map((item, index): FoundationObservation => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const quantities = [
    ...(view?.context.quantities ?? []),
    ...(view?.symbol?.quantities ?? []),
  ];
  const observation = {
    assumptions: (view?.context.assumptions ?? []).map((item) => ({
      kind: item.kind,
      subjects: item.subjects ?? [],
      value: item.value,
    })),
    caseId: item.case.id,
    conceptIds: [...new Set((view?.context.concepts ?? []).map((entry) => entry.conceptId))],
    diagnosticCodes: [...new Set((view?.diagnostics ?? []).map((entry) => entry.code))],
    definitions: (view?.symbol?.definitions ?? []).map((item) => ({
      description: item.description,
      evidenceRuleIds: [item.evidence.ruleId],
      symbol: item.symbol,
    })),
    dimensions: [...new Set(quantities.map((entry) => entry.dimension.display))],
    quantityKindIds: [...new Set(quantities.flatMap((entry) =>
      entry.quantityKindId ? [entry.quantityKindId] : [],
    ))],
    relationIds: [...new Set((view?.context.relations ?? []).map((entry) => entry.relationId))],
    ...(view ? { status: view.status } : {}),
    suiteId: item.suite.id,
    symbols: [...new Set([
      ...quantities.map((entry) => entry.symbol),
      ...(view?.symbol ? [view.symbol.symbol] : []),
    ])],
    unitIds: [...new Set(quantities.flatMap((entry) => entry.unitId ? [entry.unitId] : []))],
  };
  if (process.env.SEMATH_FOUNDATION_DEBUG?.split(",").includes(item.case.id)) {
    console.error(JSON.stringify({ item, observation, result }, null, 2));
  }
  return observation;
});

for (const suite of manifest.foundationSuites) {
  const corpus = corpora.get(suite.id)!;
  const scorecard = scoreFoundation(
    suite,
    corpus,
    observations.filter((item) => item.suiteId === suite.id),
    new Map(manifest.dimensions.map((dimension) => [dimension.id, dimension.tags])),
  );
  console.log(
    `${suite.id}: passed=${scorecard.passed}/${scorecard.cases} dimensions=${Object.entries(scorecard.dimensions).map(([id, count]) => `${id}:${count}`).join(",")} metrics=${Object.entries(scorecard.metrics).map(([id, value]) => `${id}:${value.passed}/${value.cases}`).join(",")}`,
  );
  if (scorecard.failures.length) {
    throw new Error(`foundation quality gate failed:\n${scorecard.failures.join("\n")}`);
  }
}

function languageOf(path: string): DocumentLanguage {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
