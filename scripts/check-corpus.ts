import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  type CaseObservation,
  type CorpusCase,
  type CorpusDocument,
  type MetamorphicTransform,
  evidenceIsSourceLinked,
  findCorpusDuplicates,
  planMetamorphicCases,
  rolesMatch,
  scoreQuality,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { loadQualityFixtures } from "./evaluation-fixtures";

interface PlannedCase {
  case: CorpusCase;
  generatedFrom?: {
    caseId: string;
    transform: MetamorphicTransform;
  };
  suiteId: string;
}

const { corpora, manifest } = await loadQualityFixtures();
const corpusIntegrityFailures = findCorpusDuplicates([...corpora.values()]);
if (corpusIntegrityFailures.length) {
  throw new Error(`corpus integrity gate failed:\n${corpusIntegrityFailures.join("\n")}`);
}
const planned: PlannedCase[] = manifest.suites.flatMap((suite) => {
  const corpus = corpora.get(suite.id);
  if (!corpus) throw new Error(`${suite.id}: corpus was not loaded`);
  return [...corpus.cases]
    .sort((left, right) => left.id.localeCompare(right.id))
    .map((item) => ({ case: item, suiteId: suite.id }));
});
planned.push(
  ...planMetamorphicCases(manifest, corpora).map((item) => ({
    case: item.case,
    generatedFrom: { caseId: item.sourceCaseId, transform: item.transform },
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

const fixture = {
  queries,
  snapshot: {
    documents,
    epoch: "quality-corpus",
    inventoryVersion: 1,
    projectId: "quality-corpus",
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  },
};
const native = spawnSync(
  "cargo",
  ["run", "--quiet", "--locked", "-p", "semath-native"],
  {
    encoding: "utf8",
    input: JSON.stringify(fixture),
    maxBuffer: 128 * 1024 * 1024,
  },
);
if (native.status !== 0) throw new Error(native.stderr || "native corpus run failed");
const results = JSON.parse(native.stdout) as QueryResult[];
if (results.length !== planned.length) {
  throw new Error(`native corpus returned ${results.length}/${planned.length} results`);
}

const observations = planned.map((item, index) =>
  observe(item, results[index]),
);
const scorecard = scoreQuality(manifest, corpora, observations);
for (const law of scorecard.laws) {
  console.log(
    [
      `${law.suiteId}/${law.lawId}`,
      `recall=${format(law.recall.percent)}`,
      `precision=${format(law.precision.percent)}`,
      `roles=${format(law.roleAccuracy.percent)}`,
      `evidence=${format(law.evidenceIntegrity.percent)}`,
      `refusal=${format(law.refusalPreservation.percent)}`,
      `cases=${law.positives}+${law.refusals}`,
    ].join(" "),
  );
}
console.log(
  `adversarial refusal=${format(scorecard.adversarialRefusal.percent)} cases=${scorecard.adversarialRefusal.denominator}`,
);
for (const suite of manifest.suites) {
  const dimensions = scorecard.coverage
    .filter((score) => score.suiteId === suite.id)
    .map((score) => `${score.dimension}:${score.cases}`)
    .join(",");
  console.log(`${suite.id}: dimensions=${dimensions}`);
  const diversity = scorecard.diversity
    .filter((score) => score.suiteId === suite.id)
    .map((score) =>
      score.facet === "combined-profile"
        ? `profiles:${score.distinct},max:${(score.largestShare * 100).toFixed(1)}%`
        : `${score.facet}:${score.distinct}`,
    )
    .join(",");
  console.log(`${suite.id}: diversity=${diversity}`);
}
console.log(
  `metamorphic invariance=${format(scorecard.metamorphic.percent)} cases=${scorecard.generatedCases}`,
);

if (process.env.SEMATH_CORPUS_REPORT) {
  for (const [index, observation] of observations.entries()) {
    const item = planned[index];
    if (!item) throw new Error(`missing planned corpus case at index ${index}`);
    const expected = "lawId" in item.case ? item.case.lawId : undefined;
    const targetObserved = expected
      ? observation.establishedLawIds.includes(expected)
      : false;
    if (
      expected &&
      ((item.case.expectation === "established" && !targetObserved) ||
        (item.case.expectation === "refused" && targetObserved))
    ) {
      console.error(
        `case ${item.suiteId}/${item.case.id}: expected=${item.case.expectation}:${expected} observed=${observation.establishedLawIds.join(",") || "none"}`,
      );
    }
  }
  for (const variation of scorecard.variations) {
    console.error(
      `variation ${variation.tag}: pass=${format(variation.percent)} cases=${variation.cases}`,
    );
  }
}
if (process.env.SEMATH_SCORECARD_PATH) {
  await mkdir(dirname(process.env.SEMATH_SCORECARD_PATH), { recursive: true });
  await writeFile(
    process.env.SEMATH_SCORECARD_PATH,
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
  console.log(`scorecard: ${process.env.SEMATH_SCORECARD_PATH}`);
}
if (scorecard.failures.length) {
  throw new Error(`corpus quality gate failed:\n${scorecard.failures.join("\n")}`);
}
console.log(
  `corpus quality OK: ${scorecard.authoredCases} authored cases, ${scorecard.generatedCases} generated cases, ${scorecard.variations.length} variation tags, ${scorecard.refusalCategories} refusal categories`,
);

function observe(
  item: PlannedCase,
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
  const observation: CaseObservation = {
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
  if (
    process.env.SEMATH_CORPUS_DEBUG?.split(",").includes(item.case.id)
  ) {
    console.error(JSON.stringify({ item, observation, result }, null, 2));
  }
  return observation;
}

function materializeDocuments(
  entry: CorpusCase,
  prefix: string,
): CorpusDocument[] {
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

function format(value: number): string {
  return `${value.toFixed(1)}%`;
}
