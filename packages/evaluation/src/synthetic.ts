import type {
  Corpus,
  CorpusCase,
  DiversityProfile,
  EstablishedCorpusCase,
  LawRefusalCorpusCase,
} from "./model";

export interface SyntheticDiversitySpec {
  batches: readonly string[];
  casesPerLaw: number;
  globalCases: number;
  mutationFamilies: readonly string[];
  positiveCasesPerLaw: number;
  projectTopologies: readonly string[];
  proseFamilies: readonly string[];
  schemaVersion: 1;
  syntaxStructures: readonly string[];
}

export interface PromotionSeedSuite {
  id: string;
  laws: readonly {
    lawId: string;
    positives: readonly [string, string, string, Readonly<Record<string, string>>][];
    refusals: readonly [string, string, string, string][];
  }[];
  packId: string;
}

export interface PromotionSeedSpec {
  schemaVersion: 1;
  suites: readonly PromotionSeedSuite[];
}

type LawCase = EstablishedCorpusCase | LawRefusalCorpusCase;

const BASELINE_PROSE_FAMILIES = [
  "let-singular",
  "let-paired",
  "let-series",
  "respectively",
  "where-clause",
  "we-write",
  "symbol-list",
  "role-first",
  "suppose-that",
  "given-that",
  "define-as",
  "in-this-model",
  "throughout-section",
  "compact-parenthetical",
  "separate-sentences",
  "declaration-after",
] as const;

const BASELINE_SYNTAX_STRUCTURES = [
  "inline-math",
  "display-math",
  "equation-environment",
  "aligned-environment",
  "grouped-expression",
  "styled-symbols",
  "indexed-instance",
  "macro-expanded",
] as const;

export function parseSyntheticDiversitySpec(
  value: unknown,
): SyntheticDiversitySpec {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("synthetic diversity spec must be an object");
  }
  const item = value as Record<string, unknown>;
  const allowed = [
    "schemaVersion",
    "casesPerLaw",
    "positiveCasesPerLaw",
    "globalCases",
    "batches",
    "proseFamilies",
    "projectTopologies",
    "syntaxStructures",
    "mutationFamilies",
  ];
  rejectUnknown(item, allowed, "synthetic diversity spec");
  if (item.schemaVersion !== 1) throw new Error("synthetic diversity spec version must be 1");
  const casesPerLaw = positiveInteger(item.casesPerLaw, "casesPerLaw");
  const positiveCasesPerLaw = positiveInteger(
    item.positiveCasesPerLaw,
    "positiveCasesPerLaw",
  );
  if (positiveCasesPerLaw >= casesPerLaw) {
    throw new Error("positiveCasesPerLaw must leave at least one refusal case");
  }
  return {
    batches: identifiers(item.batches, "batches", 4),
    casesPerLaw,
    globalCases: positiveInteger(item.globalCases, "globalCases"),
    mutationFamilies: identifiers(item.mutationFamilies, "mutationFamilies", 6),
    positiveCasesPerLaw,
    projectTopologies: identifiers(item.projectTopologies, "projectTopologies", 4),
    proseFamilies: identifiers(item.proseFamilies, "proseFamilies", 8),
    schemaVersion: 1,
    syntaxStructures: identifiers(item.syntaxStructures, "syntaxStructures", 4),
  };
}

export function parsePromotionSeedSpec(value: unknown): PromotionSeedSpec {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("promotion seed spec must be an object");
  }
  const root = value as Record<string, unknown>;
  rejectUnknown(root, ["schemaVersion", "suites"], "promotion seed spec");
  if (root.schemaVersion !== 1) throw new Error("promotion seed spec version must be 1");
  if (!Array.isArray(root.suites)) throw new Error("promotion seed suites must be an array");
  const suites = root.suites.map((value, suiteIndex) => {
    const path = `promotion seed suites[${suiteIndex}]`;
    const suite = record(value, path);
    rejectUnknown(suite, ["id", "packId", "laws"], path);
    if (!Array.isArray(suite.laws) || !suite.laws.length) {
      throw new Error(`${path}.laws must be a nonempty array`);
    }
    return {
      id: checkedIdentifier(suite.id, `${path}.id`),
      laws: suite.laws.map((value, lawIndex) => {
        const lawPath = `${path}.laws[${lawIndex}]`;
        const law = record(value, lawPath);
        rejectUnknown(law, ["lawId", "positives", "refusals"], lawPath);
        return {
          lawId: checkedIdentifier(law.lawId, `${lawPath}.lawId`),
          positives: positiveSeedRows(law.positives, `${lawPath}.positives`),
          refusals: refusalSeedRows(law.refusals, `${lawPath}.refusals`),
        };
      }),
      packId: checkedIdentifier(suite.packId, `${path}.packId`),
    };
  });
  if (new Set(suites.map((suite) => suite.id)).size !== suites.length) {
    throw new Error("promotion seed suite ids must be unique");
  }
  return { schemaVersion: 1, suites };
}

export function buildPromotionSeedCorpus(suite: PromotionSeedSuite): Corpus {
  const cases = suite.laws.flatMap((law) => [
    ...law.positives.map(([id, content, needle, expectedRoles]): CorpusCase => ({
      cursor: { fileId: "main", needle },
      diversity: placeholderDiversity,
      documents: [{
        content,
        fileId: "main",
        path: "main.md",
      }],
      expectation: "established",
      expectedRoles,
      id,
      lawId: law.lawId,
      variationTags: [
        "conventional-notation",
        "english-declarations",
        "role-prose",
        "shape-explicit",
      ],
    })),
    ...law.refusals.map(([id, content, needle, refusalCategory]): CorpusCase => ({
      cursor: { fileId: "main", needle },
      diversity: placeholderDiversity,
      documents: [{
        content,
        fileId: "main",
        path: "main.md",
      }],
      expectation: "refused",
      id,
      lawId: law.lawId,
      refusalCategory,
      variationTags: ["hard-negative", "role-prose", refusalCategory],
    })),
  ]);
  return { cases, domain: suite.id, schemaVersion: 2 };
}

const placeholderDiversity: DiversityProfile = {
  batch: "promotion-seed",
  mutationFamily: "unclassified",
  projectTopology: "single-document",
  proseFamily: "unclassified",
  semanticSkeleton: "unclassified",
  syntaxStructure: "inline-math",
};

export function annotateCorpus(corpus: Corpus): Corpus {
  return {
    ...corpus,
    cases: corpus.cases.map((item, index) => {
      const proseFamily = BASELINE_PROSE_FAMILIES[index % BASELINE_PROSE_FAMILIES.length]!;
      const syntax = BASELINE_SYNTAX_STRUCTURES[index % BASELINE_SYNTAX_STRUCTURES.length]!;
      return {
        ...item,
        cursor: { ...item.cursor, edge: index % 2 ? "after" : "before" },
        diversity: baselineDiversity(item, index, proseFamily, syntax),
        documents: item.documents.map((document) =>
          document.fileId === item.cursor.fileId
            ? {
                ...document,
                content: `${neutralPrelude(proseFamily, index + 1000)}\n${syntaxExample(syntax, index + 1000, "baselineprobe")}\n\n${stripGeneratedBaselineDecoration(document.content)}`,
              }
            : document,
        ),
      };
    }),
    schemaVersion: 2,
  };
}

export function generateLawDiversityCorpus(
  domain: string,
  sourceCases: readonly CorpusCase[],
  spec: SyntheticDiversitySpec,
): Corpus {
  const lawCases = sourceCases.filter((item): item is LawCase => "lawId" in item);
  const lawIds = [...new Set(lawCases.map((item) => item.lawId))].sort();
  const cases = lawIds.flatMap((lawId) => {
    const candidates = lawCases.filter((item) => item.lawId === lawId);
    const positives = candidates.filter(
      (item): item is EstablishedCorpusCase => item.expectation === "established",
    );
    const refusals = candidates.filter(
      (item): item is LawRefusalCorpusCase => item.expectation === "refused",
    );
    if (!positives.length || !refusals.length) {
      throw new Error(`${lawId}: generation requires positive and refusal source cases`);
    }
    return Array.from({ length: spec.casesPerLaw }, (_, index) => {
      const established = index < spec.positiveCasesPerLaw;
      const pool = established ? positives : refusals;
      const source = pool[(index * 7 + Math.floor(index / 3)) % pool.length]!;
      return synthesizeLawCase(source, lawId, index, spec);
    });
  });
  return { cases, domain, schemaVersion: 2 };
}

export function generateGlobalRefusalCorpus(
  domain: string,
  spec: SyntheticDiversitySpec,
): Corpus {
  const variables = [
    ["z", "q", "r"],
    ["h", "j", "k"],
    ["w", "s", "d"],
    ["g", "b", "c"],
    ["n", "u", "e"],
  ] as const;
  const operators = ["+", "-", "\\cdot", "/", "\\neq"] as const;
  const cases: CorpusCase[] = Array.from({ length: spec.globalCases }, (_, index) => {
    const [left, first, second] = variables[index % variables.length]!;
    const suffix = index + 11;
    const expression = `${left}_{${suffix}}=${first}_{${suffix}}${operators[index % operators.length]}${second}_{${suffix}}`;
    const proseFamily = spec.proseFamilies[(index * 3) % spec.proseFamilies.length]!;
    const content = `${neutralPrelude(proseFamily, index)}\n\n$${expression}$`;
    return {
      cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle: expression },
      diversity: diversityFor(index, spec, `unsupported-${index % 10}`),
      documents: [{ content, fileId: "main", path: index % 2 ? "main.md" : "main.tex" }],
      expectation: "refused",
      id: `synthetic-unknown-${String(index + 1).padStart(2, "0")}`,
      refusalCategory: index % 2 ? "unknown-domain-relation" : "cross-pack-symbol-collision",
      variationTags: [
        "hard-negative",
        "safe-refusal",
        index % 2 ? "unknown-domain" : "notation-collision",
      ],
    };
  });
  return { cases, domain, schemaVersion: 2 };
}

export function findCorpusDuplicates(corpora: readonly Corpus[]): string[] {
  const failures: string[] = [];
  const seen = new Map<string, string>();
  const profiles = new Map<string, string>();
  for (const corpus of corpora) {
    for (const item of corpus.cases) {
      const key = item.documents
        .map((document) => normalizeSource(document.content))
        .sort()
        .join("\n--document--\n");
      const previous = seen.get(key);
      if (previous) failures.push(`${corpus.domain}/${item.id}: duplicates ${previous}`);
      else seen.set(key, `${corpus.domain}/${item.id}`);
      if (item.documents.some((document) => document.content.includes(item.id))) {
        failures.push(`${corpus.domain}/${item.id}: source leaks fixture identity`);
      }
      for (const document of item.documents) {
        failures.push(
          ...validateFixtureSource(document.content).map(
            (failure) => `${corpus.domain}/${item.id}/${document.fileId}: ${failure}`,
          ),
        );
      }
      const profile = [
        "lawId" in item ? item.lawId : "global-refusal",
        item.diversity.semanticSkeleton,
        item.diversity.syntaxStructure,
        item.diversity.proseFamily,
        item.diversity.projectTopology,
        item.diversity.mutationFamily,
        proseNgramSignature(item.documents.map((document) => document.content).join("\n")),
        [...item.variationTags].sort().join(","),
      ].join("\u0000");
      const previousProfile = profiles.get(profile);
      if (previousProfile) {
        failures.push(
          `${corpus.domain}/${item.id}: duplicates semantic/syntax/prose/tag profile of ${previousProfile}`,
        );
      } else {
        profiles.set(profile, `${corpus.domain}/${item.id}`);
      }
    }
  }
  return failures.sort();
}

export function validateFixtureSource(source: string): string[] {
  const failures: string[] = [];
  if (source.includes("\0")) failures.push("contains a NUL byte");
  const dollars = source.match(/(?<!\\)\$/gu)?.length ?? 0;
  if (dollars % 2 !== 0) failures.push("has an unmatched math dollar delimiter");
  if ((source.match(/\\\[/gu)?.length ?? 0) !== (source.match(/\\\]/gu)?.length ?? 0)) {
    failures.push("has unmatched display-math delimiters");
  }
  const environmentStack: string[] = [];
  for (const match of source.matchAll(/\\(begin|end)\{([^}]+)\}/gu)) {
    if (match[1] === "begin") environmentStack.push(match[2]!);
    else if (environmentStack.pop() !== match[2]) {
      failures.push("has mismatched TeX environments");
      break;
    }
  }
  if (environmentStack.length && !failures.includes("has mismatched TeX environments")) {
    failures.push("has mismatched TeX environments");
  }
  const fences = source.match(/^\s*```/gmu)?.length ?? 0;
  if (fences % 2 !== 0) failures.push("has an unmatched Markdown fence");
  return failures;
}

function synthesizeLawCase(
  source: LawCase,
  lawId: string,
  index: number,
  spec: SyntheticDiversitySpec,
): LawCase {
  const proseFamily = spec.proseFamilies[(index * 5 + Math.floor(index / 7)) % spec.proseFamilies.length]!;
  const projectTopology = spec.projectTopologies[(index * 3 + Math.floor(index / 11)) % spec.projectTopologies.length]!;
  const diversity = diversityFor(
    index,
    spec,
    semanticSkeleton(source, index),
    projectTopology,
    proseFamily,
  );
  const prefix = neutralPrelude(proseFamily, index);
  const documents = source.documents.map((document) =>
    document.fileId === source.cursor.fileId
      ? {
          ...document,
          content: `${prefix}\n${syntaxExample(diversity.syntaxStructure, index)}\n\n${document.content}`,
        }
      : { ...document },
  );
  if (projectTopology !== "single-document") {
    documents.push({
      content: contextDocument(projectTopology, proseFamily, index),
      fileId: `context-${index}`,
      path: contextPath(projectTopology, index),
    });
  }
  const expectation = source.expectation;
  const ordinal = String(index + 1).padStart(2, "0");
  return {
    ...source,
    cursor: { ...source.cursor, edge: index % 2 ? "after" : "before" },
    diversity,
    documents,
    id: `synthetic-${lawId}-${expectation}-${ordinal}`,
    variationTags: [
      ...new Set([
        ...source.variationTags,
        "synthetic-authored",
        `prose-${proseFamily}`,
        `topology-${projectTopology}`,
        ...(projectTopology === "single-document" ? [] : ["multi-file"]),
        ...(diversity.syntaxStructure === "macro-expanded" ? ["macro"] : []),
      ]),
    ],
  };
}

function baselineDiversity(
  item: CorpusCase,
  index: number,
  proseFamily: string,
  syntax: string,
): DiversityProfile {
  const document = item.documents.find((candidate) => candidate.fileId === item.cursor.fileId)!;
  const topology = item.documents.length > 1
    ? "multiple-documents"
    : item.macros?.length
      ? "macro-context"
      : "single-document";
  const mutation = item.expectation === "refused"
    ? item.refusalCategory
    : "affirmative";
  return {
    batch: `baseline-${String((index % 8) + 1).padStart(2, "0")}`,
    mutationFamily: safeIdentifier(mutation),
    projectTopology: topology,
    proseFamily,
    semanticSkeleton: semanticSkeleton(item, index),
    syntaxStructure: syntax,
  };
}

function diversityFor(
  index: number,
  spec: SyntheticDiversitySpec,
  skeleton: string,
  topology = spec.projectTopologies[(index * 3) % spec.projectTopologies.length]!,
  prose = spec.proseFamilies[(index * 5) % spec.proseFamilies.length]!,
): DiversityProfile {
  return {
    batch: spec.batches[(index * 7 + Math.floor(index / 9)) % spec.batches.length]!,
    mutationFamily: spec.mutationFamilies[(index * 5 + Math.floor(index / 7)) % spec.mutationFamilies.length]!,
    projectTopology: topology,
    proseFamily: prose,
    semanticSkeleton: safeIdentifier(skeleton),
    syntaxStructure: spec.syntaxStructures[(index * 7 + Math.floor(index / 5)) % spec.syntaxStructures.length]!,
  };
}

function neutralPrelude(family: string, index: number): string {
  const variants = [
    `Consider the following independently stated relation in example ${index + 1}.`,
    `For comparison, inspect the displayed expression numbered ${index + 1}.`,
    `The next formula is presented without an assumed named law (${index + 1}).`,
    `In this local discussion, only the explicit statement below is available (${index + 1}).`,
    `Suppose the notation is local to this calculation, case ${index + 1}.`,
    `The symbols below have no meaning beyond this standalone example (${index + 1}).`,
    `As a deliberately unsupported instance, examine relation ${index + 1}.`,
    `No domain interpretation is asserted for the following formula (${index + 1}).`,
  ];
  return `${variants[index % variants.length]} [${family.replaceAll("-", " ")}]`;
}

function semanticSkeleton(item: CorpusCase, index: number): string {
  const needle = item.cursor.needle
    .replace(/\\[A-Za-z]+/gu, " command ")
    .replace(/[A-Za-z]+/gu, " symbol ")
    .replace(/\d+/gu, " number ")
    .replace(/[^A-Za-z0-9]+/gu, "-")
    .replace(/^-+|-+$/gu, "")
    .slice(0, 42);
  const mutation = item.expectation === "refused" ? item.refusalCategory : "established";
  return safeIdentifier(`${mutation}-${needle || `form-${index % 12}`}`);
}

function syntaxStructure(path: string, tags: readonly string[], index: number): string {
  const language = path.endsWith(".md") ? "markdown" : "latex";
  const form = tags.find((tag) => /inline|display|align|equation|macro|group/u.test(tag));
  return safeIdentifier(`${language}-${form ?? `form-${index % 6}`}`);
}

function contextDocument(topology: string, proseFamily: string, index: number): string {
  return [
    `Supporting note ${index + 1}: this ${topology.replaceAll("-", " ")} file is intentionally outside the formula's declaration scope.`,
    surfaceDeclaration(proseFamily, index),
  ].join("\n");
}

function surfaceDeclaration(family: string, index: number): string {
  const suffix = index + 101;
  const x = `$x_{${suffix}}$`;
  const y = `$y_{${suffix}}$`;
  const z = `$z_{${suffix}}$`;
  switch (family) {
    case "let-singular":
      return `Let ${x} be the local input.`;
    case "let-paired":
      return `Let ${x} and ${y} denote the local input and output, respectively.`;
    case "let-series":
      return `Let ${x}, ${y}, and ${z} denote the local input, state, and output, respectively.`;
    case "respectively":
      return `The symbols ${x}, ${y}, and ${z} stand for the datum, estimate, and residual, respectively.`;
    case "where-clause":
      return `Here ${x} denotes the input, where ${y} represents the output.`;
    case "we-write":
      return `We write ${x} for the input and ${y} for the output.`;
    case "symbol-list":
      return `The symbols ${x} and ${y} represent the source and target, in that order.`;
    case "role-first":
      return `The local input and output are denoted by ${x} and ${y}, respectively.`;
    case "suppose-that":
      return `Suppose that ${x} is the observed input and ${y} is the predicted output.`;
    case "given-that":
      return `Given ${x} as the input, take ${y} to represent the output.`;
    case "define-as":
      return `Define ${x} as the sample and ${y} as the estimate.`;
    case "in-this-model":
      return `In this model, ${x}, ${y}, and ${z} mean the sample, estimate, and error, respectively.`;
    case "throughout-section":
      return `Throughout this section, ${x} stands for the independent variable.`;
    case "compact-parenthetical":
      return `The normalized vector (${x}) is fixed locally.`;
    case "separate-sentences":
      return `${x} denotes the source. ${y} denotes the target. ${z} denotes the residual.`;
    default:
      return `The local output is represented by ${y} after the formula.`;
  }
}

function syntaxExample(
  structure: string,
  index: number,
  macroName = "syntheticprobe",
): string {
  const symbol = `q_{${index + 201}}`;
  switch (structure) {
    case "inline-math":
      return `A presentation-only symbol is $${symbol}$.`;
    case "display-math":
      return `A presentation-only symbol follows: \\[${symbol}\\]`;
    case "equation-environment":
      return `\\begin{equation}${symbol}\\end{equation}`;
    case "aligned-environment":
      return `\\[\\begin{aligned}${symbol}\\end{aligned}\\]`;
    case "grouped-expression":
      return `A grouped symbol is $ {${symbol}} $.`;
    case "styled-symbols":
      return `A styled symbol is $\\mathbf{q}_{${index + 201}}$.`;
    case "indexed-instance":
      return `An indexed symbol is $${symbol}$.`;
    default:
      return `\\newcommand{\\${macroName}}{${symbol}}A macro-expanded symbol is $\\${macroName}$.`;
  }
}

function stripGeneratedBaselineDecoration(content: string): string {
  const marker = "\n\n";
  if (!content.startsWith("Consider the following independently stated relation") &&
      !content.startsWith("For comparison, inspect") &&
      !content.startsWith("The next formula is presented") &&
      !content.startsWith("In this local discussion") &&
      !content.startsWith("Suppose the notation is local") &&
      !content.startsWith("The symbols below have no meaning") &&
      !content.startsWith("As a deliberately unsupported instance") &&
      !content.startsWith("No domain interpretation is asserted")) {
    return content;
  }
  const separator = content.indexOf(marker);
  return separator < 0 ? content : content.slice(separator + marker.length);
}

function contextPath(topology: string, index: number): string {
  if (topology === "nested-section") return `sections/context-${index}.md`;
  if (topology === "appendix-file") return `appendices/note-${index}.tex`;
  if (topology === "definitions-file") return `definitions-${index}.md`;
  return `context-${index}.md`;
}

function normalizeSource(value: string): string {
  return value
    .replace(/<!--.*?-->/gsu, " ")
    .replace(/%[^\n]*/gu, " ")
    .replace(/\s+/gu, " ")
    .trim();
}

function proseNgramSignature(value: string): string {
  const words = value
    .replace(/\\[A-Za-z]+/gu, " ")
    .replace(/\$[^$]*\$/gu, " ")
    .toLowerCase()
    .match(/[a-z]{3,}/gu) ?? [];
  const ngrams = new Set<string>();
  for (let index = 0; index + 2 < words.length; index += 1) {
    ngrams.add(words.slice(index, index + 3).join("-"));
  }
  return [...ngrams].sort().slice(0, 12).join("|");
}

function safeIdentifier(value: string): string {
  const result = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, "-")
    .replace(/^-+|-+$/gu, "")
    .replace(/^[^a-z]+/u, "");
  return result || "unclassified";
}

function identifiers(
  value: unknown,
  path: string,
  minimum: number,
): string[] {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array`);
  const result = value.map((item, index) => {
    if (typeof item !== "string" || safeIdentifier(item) !== item) {
      throw new Error(`${path}[${index}] must be a lowercase kebab-case identifier`);
    }
    return item;
  });
  if (new Set(result).size !== result.length) throw new Error(`${path} contains duplicates`);
  if (result.length < minimum) throw new Error(`${path} requires at least ${minimum} values`);
  return result;
}

function positiveSeedRows(
  value: unknown,
  path: string,
): [string, string, string, Readonly<Record<string, string>>][] {
  if (!Array.isArray(value) || value.length !== 5) {
    throw new Error(`${path} must contain exactly five rows`);
  }
  return value.map((row, index) => {
    const rowPath = `${path}[${index}]`;
    if (!Array.isArray(row) || row.length !== 4) {
      throw new Error(`${rowPath} must be [id, content, needle, expectedRoles]`);
    }
    const roles = record(row[3], `${rowPath}[3]`);
    const expectedRoles = Object.fromEntries(
      Object.entries(roles).map(([role, symbol]) => [
        checkedIdentifier(role, `${rowPath}[3].${role}`),
        checkedText(symbol, `${rowPath}[3].${role}`),
      ]),
    );
    if (!Object.keys(expectedRoles).length) throw new Error(`${rowPath}[3] must not be empty`);
    return [
      checkedIdentifier(row[0], `${rowPath}[0]`),
      checkedText(row[1], `${rowPath}[1]`),
      checkedText(row[2], `${rowPath}[2]`),
      expectedRoles,
    ];
  });
}

function refusalSeedRows(
  value: unknown,
  path: string,
): [string, string, string, string][] {
  if (!Array.isArray(value) || value.length !== 5) {
    throw new Error(`${path} must contain exactly five rows`);
  }
  return value.map((row, index) => {
    const rowPath = `${path}[${index}]`;
    if (!Array.isArray(row) || row.length !== 4) {
      throw new Error(`${rowPath} must be [id, content, needle, refusalCategory]`);
    }
    return [
      checkedIdentifier(row[0], `${rowPath}[0]`),
      checkedText(row[1], `${rowPath}[1]`),
      checkedText(row[2], `${rowPath}[2]`),
      checkedIdentifier(row[3], `${rowPath}[3]`),
    ];
  });
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function checkedText(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path} must be a nonempty string`);
  }
  return value;
}

function checkedIdentifier(value: unknown, path: string): string {
  const result = checkedText(value, path);
  if (safeIdentifier(result) !== result) {
    throw new Error(`${path} must be a lowercase kebab-case identifier`);
  }
  return result;
}

function positiveInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value) || (value as number) < 1) {
    throw new Error(`${path} must be a positive integer`);
  }
  return value as number;
}

function rejectUnknown(
  item: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
): void {
  const keys = Object.keys(item).filter((key) => !allowed.includes(key));
  if (keys.length) throw new Error(`${path}: unknown fields: ${keys.sort().join(", ")}`);
}
