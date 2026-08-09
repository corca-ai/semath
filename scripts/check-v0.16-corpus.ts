import { readdir, readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";

type CorpusDocument = { content: string; fileId: string; path: string };
type CorpusMacro = {
  body?: string;
  definition?: string;
  name: string;
  parameterCount?: number;
};
type CorpusCase = {
  cursor: { edge?: "after" | "before"; fileId: string; needle: string };
  documents: CorpusDocument[];
  expectation: "established" | "refused";
  expectedRoles?: Record<string, string>;
  id: string;
  lawId: string;
  macros?: CorpusMacro[];
  refusalCategory?: string;
  variationTags: string[];
};
type Corpus = {
  cases: CorpusCase[];
  domain: string;
  schemaVersion: number;
};

const root = new URL("../fixtures/v0.16/", import.meta.url);
const names = (await readdir(root))
  .filter((name) => name.endsWith(".json"))
  .sort();
if (names.length !== 3) {
  throw new Error(`v0.16 corpus requires exactly three independent domains, got ${names.length}`);
}
const architectureCorpora = await Promise.all(
  names.map(async (name) => {
    const corpus = JSON.parse(await readFile(new URL(name, root), "utf8")) as Corpus;
    validateCorpus(corpus, name);
    return corpus;
  }),
);
const blindRoot = new URL("../fixtures/v0.16/blind-extension/", import.meta.url);
const blindNames = (await readdir(blindRoot)).filter((name) => name.endsWith(".json")).sort();
const blindCorpora = await Promise.all(
  blindNames.map(async (name) => {
    const corpus = JSON.parse(await readFile(new URL(name, blindRoot), "utf8")) as Corpus;
    validateCorpus(corpus, `blind-extension/${name}`);
    return corpus;
  }),
);
const corpora = [...architectureCorpora, ...blindCorpora];

const documents: ProjectDocument[] = [];
const queries: QueryEnvelope[] = [];
const expectations: Array<{ corpus: Corpus; entry: CorpusCase }> = [];
for (const corpus of corpora) {
  for (const entry of corpus.cases) {
    const prefix = `${corpus.domain}/${entry.id}/`;
    const inputs = materializeDocuments(entry, prefix);
    const syntax = new LatexSyntaxService();
    syntax.reset({
      documents: inputs
        .filter((document) => languageOf(document.path) !== "bibtex")
        .map((document) => ({ ...document, documentVersion: 1 })),
    });
    for (const input of inputs) {
      const language = languageOf(input.path);
      const fileId = input.fileId;
      if (language === "bibtex") {
        documents.push({
          ...input,
          documentVersion: 1,
          fileId,
          includes: [],
          language,
          macros: [],
          mathRegions: [],
        });
        continue;
      }
      const snapshot = syntax.getFile(fileId);
      if (!snapshot) throw new Error(`${corpus.domain}/${entry.id}: missing syntax snapshot`);
      if (process.env.SEMATH_CORPUS_DEBUG?.split(",").includes(entry.id)) {
        console.error(JSON.stringify({ id: entry.id, fileId, syntaxMacros: snapshot.macros }, null, 2));
      }
      documents.push(adaptWasmtexDocument({ content: input.content, language, syntax: snapshot }));
    }
    const cursorDocument = inputs.find(
      (document) => document.fileId === prefix + entry.cursor.fileId,
    );
    if (!cursorDocument) throw new Error(`${corpus.domain}/${entry.id}: unknown cursor file`);
    const first = cursorDocument.content.indexOf(entry.cursor.needle);
    const last = cursorDocument.content.lastIndexOf(entry.cursor.needle);
    if (first < 0 || first !== last) {
      throw new Error(`${corpus.domain}/${entry.id}: cursor needle must occur exactly once`);
    }
    const offset = entry.cursor.edge === "after" ? first + entry.cursor.needle.length : first;
    queries.push({
      analysisGeneration: 0,
      documentVersion: 1,
      epoch: "v0.16-corpus",
      inventoryVersion: 1,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query: {
        fileId: prefix + entry.cursor.fileId,
        kind: "semanticView",
        offset,
      },
    });
    expectations.push({ corpus, entry });
  }
}

const fixture = {
  queries,
  snapshot: {
    documents,
    epoch: "v0.16-corpus",
    inventoryVersion: 1,
    projectId: "v0.16-corpus",
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  },
};
const native = spawnSync("cargo", ["run", "--quiet", "--locked", "-p", "semath-native"], {
  encoding: "utf8",
  input: JSON.stringify(fixture),
  maxBuffer: 128 * 1024 * 1024,
});
if (native.status !== 0) throw new Error(native.stderr || "v0.16 native corpus failed");
if (process.env.SEMATH_CORPUS_DEBUG && native.stderr) console.error(native.stderr);
const results = JSON.parse(native.stdout) as QueryResult[];
if (process.env.SEMATH_CORPUS_DEBUG) {
  for (const id of process.env.SEMATH_CORPUS_DEBUG.split(",")) {
    const index = expectations.findIndex(({ entry }) => entry.id === id);
    if (index >= 0) console.error(JSON.stringify({ id, result: results[index] }, null, 2));
  }
}

const cells = new Map<
  string,
  { falsePositive: number; positive: number; recognized: number; refused: number }
>();
const variations = new Map<
  string,
  { falsePositive: number; positive: number; recognized: number; refused: number }
>();
const refusalCategories = new Map<string, number>();
const failures: string[] = [];
const misses: string[] = [];
const missedIds = new Map<string, string[]>();
const falsePositiveIds = new Map<string, string[]>();
for (const [index, { corpus, entry }] of expectations.entries()) {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const relation = view?.context.relations.find((candidate) =>
    candidate.relationId.endsWith(`:${entry.lawId}`),
  );
  const key = `${corpus.domain}/${entry.lawId}`;
  const cell = cells.get(key) ?? { falsePositive: 0, positive: 0, recognized: 0, refused: 0 };
  const variationCells = entry.variationTags.map((tag) => [
    tag,
    variations.get(tag) ?? { falsePositive: 0, positive: 0, recognized: 0, refused: 0 },
  ] as const);
  if (entry.expectation === "established") {
    cell.positive++;
    variationCells.forEach(([, value]) => value.positive++);
    if (view?.status === "established" && relation) {
      cell.recognized++;
      variationCells.forEach(([, value]) => value.recognized++);
      if (
        relation.evidence.length === 0 ||
        relation.evidence.some((item) => item.sourceRanges.length === 0) ||
        relation.roles.length === 0 ||
        relation.conditions.length === 0
      ) {
        failures.push(`${key}/${entry.id}: recognition is missing source-linked evidence`);
      }
      const actualRoles = Object.fromEntries(
        relation.roles.flatMap((role) => {
          const keys = [role.role];
          if (role.conceptId) keys.push(role.conceptId.split(":").at(-1)!);
          return keys.map((key) => [normalizeRole(key), role.symbol]);
        }),
      );
      const variadic = relation.roles.filter((role) => role.role === "branch-current");
      if (variadic.length) {
        const actual = new Set(
          variadic.map((role) => normalizeSymbol(role.symbol, entry.macros)),
        );
        const expected = new Set(
          Object.values(entry.expectedRoles ?? {}).map((symbol) => normalizeSymbol(symbol, entry.macros)),
        );
        if (actual.size !== expected.size || [...expected].some((symbol) => !actual.has(symbol))) {
          failures.push(`${key}/${entry.id}: expected currents ${[...expected]}, got ${[...actual]}`);
        }
      } else {
        for (const [role, symbol] of Object.entries(entry.expectedRoles ?? {})) {
          const actual = actualRoles[normalizeRole(role)];
          if (normalizeSymbol(actual ?? "", entry.macros) !== normalizeSymbol(symbol, entry.macros)) {
            failures.push(
              `${key}/${entry.id}: role ${role} expected ${symbol}, got ${actual ?? "missing"}`,
            );
          }
        }
      }
    } else {
      if (misses.length < 20) {
        misses.push(
          `${key}/${entry.id}: status=${view?.status ?? "missing"} relations=${view?.context.relations.map((item) => item.relationId).join(",") ?? "none"}`,
        );
      }
      (missedIds.get(key) ?? missedIds.set(key, []).get(key)!).push(entry.id);
    }
  } else if (relation) {
    cell.falsePositive++;
    variationCells.forEach(([, value]) => value.falsePositive++);
    (falsePositiveIds.get(key) ?? falsePositiveIds.set(key, []).get(key)!).push(entry.id);
  } else {
    cell.refused++;
    variationCells.forEach(([, value]) => value.refused++);
    if (!entry.refusalCategory) {
      failures.push(`${key}/${entry.id}: refusal has no authored category`);
    } else {
      refusalCategories.set(
        entry.refusalCategory,
        (refusalCategories.get(entry.refusalCategory) ?? 0) + 1,
      );
    }
  }
  cells.set(key, cell);
  variationCells.forEach(([tag, value]) => variations.set(tag, value));
}
if (process.env.SEMATH_CORPUS_REPORT) {
  for (const key of [...cells.keys()].sort()) {
    console.error(`${key} misses: ${(missedIds.get(key) ?? []).join(", ")}`);
    console.error(`${key} false positives: ${(falsePositiveIds.get(key) ?? []).join(", ")}`);
  }
}
const variationScores = [...variations].map(([tag, cell]) => ({
  tag,
  precision: percent(cell.recognized, cell.recognized + cell.falsePositive),
  recall: percent(cell.recognized, cell.positive),
}));
if (process.env.SEMATH_CORPUS_REPORT) {
  for (const score of variationScores.sort((left, right) => left.tag.localeCompare(right.tag))) {
    console.error(
      `variation ${score.tag}: recall=${score.recall.toFixed(1)}% precision=${score.precision.toFixed(1)}%`,
    );
  }
}

for (const [key, cell] of [...cells].sort()) {
  const recall = percent(cell.recognized, cell.positive);
  const precision = percent(cell.recognized, cell.recognized + cell.falsePositive);
  console.log(
    `v0.16 corpus ${key}: recall=${recall.toFixed(1)}% precision=${precision.toFixed(1)}% positives=${cell.positive} refusals=${cell.refused}`,
  );
  if (recall < 95) failures.push(`${key}: recall ${recall.toFixed(1)}% is below 95%`);
  if (precision < 99) failures.push(`${key}: precision ${precision.toFixed(1)}% is below 99%`);
}
if (failures.length) {
  throw new Error(`v0.16 corpus gate failed:\n${failures.join("\n")}\nSample misses:\n${misses.join("\n")}`);
}
console.log(
  `v0.16 corpus OK: ${expectations.length} independently authored cases, ${variations.size} variation tags, ${refusalCategories.size} refusal categories`,
);

function validateCorpus(corpus: Corpus, name: string): void {
  if (corpus.schemaVersion !== 1 || !corpus.domain || !Array.isArray(corpus.cases)) {
    throw new Error(`${name}: invalid corpus envelope`);
  }
  const ids = new Set<string>();
  const tags = new Set<string>();
  for (const entry of corpus.cases) {
    if (ids.has(entry.id)) throw new Error(`${name}: duplicate case ${entry.id}`);
    ids.add(entry.id);
    entry.variationTags.forEach((tag) => tags.add(tag));
    if (!entry.documents.length) throw new Error(`${name}/${entry.id}: no documents`);
    if (entry.expectation === "established" && !entry.expectedRoles) {
      throw new Error(`${name}/${entry.id}: positive case has no expected roles`);
    }
    if (entry.expectation === "refused" && !entry.refusalCategory) {
      throw new Error(`${name}/${entry.id}: negative case has no refusal category`);
    }
  }
  if (tags.size < 12) throw new Error(`${name}: requires at least 12 variation tags`);
}

function materializeDocuments(entry: CorpusCase, prefix: string): CorpusDocument[] {
  const inputs = entry.documents.map((document) => ({
    ...document,
    fileId: prefix + document.fileId,
    path: prefix + document.path,
  }));
  const missing = (entry.macros ?? []).filter(
    (macro) => !inputs.some((document) => document.content.includes(`\\newcommand{${macro.name}}`)),
  );
  if (missing.length) {
    const main = inputs.find((document) => document.fileId === prefix + entry.cursor.fileId);
    if (!main) throw new Error(`${entry.id}: macro case has no main document`);
    const preamble = missing
      .map(
        (macro) =>
          `\\newcommand{${macro.name}}${macro.parameterCount ? `[${macro.parameterCount}]` : ""}{${macro.definition ?? macro.body ?? ""}}`,
      )
      .join("\n");
    main.content = `${preamble}\n${main.content}`;
  }
  return inputs;
}

function languageOf(path: string): DocumentLanguage {
  if (/\.md$/i.test(path)) return "markdown";
  if (/\.bib$/i.test(path)) return "bibtex";
  return "latex";
}

function percent(numerator: number, denominator: number): number {
  return denominator === 0 ? 100 : (numerator / denominator) * 100;
}

function normalizeRole(role: string): string {
  return ({ "net-force": "force", speed: "velocity" } as Record<string, string>)[role] ?? role;
}

function normalizeSymbol(symbol: string, macros: readonly CorpusMacro[] | undefined): string {
  let value = symbol;
  for (const macro of macros ?? []) {
    const expansion = macro.definition ?? macro.body ?? "";
    value = value
      .replaceAll(macro.name, expansion)
      .replaceAll(macro.name.replace(/^\\/, ""), expansion);
  }
  for (;;) {
    const next = value.replace(
      /\\(?:mathbf|boldsymbol|vec|mathcal|mathrm|mathit|tilde)\{([^{}]+)\}/g,
      "$1",
    );
    if (next === value) break;
    value = next;
  }
  return value
    .replace(/\^\{\(1\)\}/g, "")
    .replace(/_\{([^{}]+)\}/g, "_$1")
    .replace(/\\(?:rm|mathbf|boldsymbol|vec|mathcal|mathrm|mathit|tilde)\s*/g, "")
    .replace(/\\([A-Za-z]+)/g, "$1")
    .replace(/[{}\s]/g, "");
}
