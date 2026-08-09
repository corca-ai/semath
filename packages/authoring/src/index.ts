import {
  CAPABILITY_IDS,
  type Corpus,
  type CorpusCase,
  type QualityManifest,
  type QualityScorecard,
} from "../../evaluation/src/index";

export interface PackAuthoringDiagnostic {
  code: string;
  entityId?: string;
  file: string;
  jsonPath: string;
  message: string;
  severity: "error" | "warning";
}

export interface PackCanonicalForm {
  canonical: string;
  formIndex: number;
  lawId: string;
  packId: string;
  source: string;
}

export interface PackAuthoringReport {
  diagnostics: readonly PackAuthoringDiagnostic[];
  forms: readonly PackCanonicalForm[];
  packs: readonly {
    concepts: number;
    laws: number;
    packId: string;
    packVersion: string;
    quantityKinds: number;
    units: number;
  }[];
  schemaVersion: 1;
}

export interface PackAuthoringRequest {
  schemaVersion: 1;
  sources: readonly { path: string; source: string }[];
}

export interface AuthoringPack {
  laws: readonly AuthoringLaw[];
  packId: string;
  title: string;
}

export interface AuthoringLaw {
  id: string;
  roles: readonly { id: string; shape?: string }[];
  semanticForms: readonly string[];
  title: string;
}

export interface PackWorkspaceScaffold {
  corpus: Corpus;
  manifest: QualityManifest;
}

export interface RuntimeSource {
  path: string;
  source: string;
}

export interface RuntimeBranchViolation {
  id: string;
  line: number;
  path: string;
  sourceLine: string;
}

export interface ScorecardComparison {
  improvements: readonly string[];
  regressions: readonly string[];
  unchanged: readonly string[];
}

export function projectValidatedPack(value: unknown): AuthoringPack {
  const pack = value as {
    laws: { id: string; roles: { id: string; shape?: string }[]; semanticForms: string[]; title: string }[];
    packId: string;
    title: string;
  };
  return {
    laws: pack.laws.map((law) => ({
      id: law.id,
      roles: law.roles.map((role) => ({
        id: role.id,
        ...(role.shape ? { shape: role.shape } : {}),
      })),
      semanticForms: [...law.semanticForms],
      title: law.title,
    })),
    packId: pack.packId,
    title: pack.title,
  };
}

export function scaffoldPackWorkspace(pack: AuthoringPack): PackWorkspaceScaffold {
  const suiteId = `${pack.packId}-probe`;
  const corpus: Corpus = {
    cases: pack.laws.flatMap((law) => scaffoldLawCases(law)),
    domain: suiteId,
    schemaVersion: 2,
  };
  const unsupported = { maturity: "unsupported", suiteIds: [] } as const;
  const probe = { maturity: "probe", suiteIds: [suiteId] } as const;
  const manifest: QualityManifest = {
    dimensions: [
      {
        id: "notation",
        tags: ["conventional-notation", "display-notation", "grouped"],
      },
      {
        id: "prose",
        tags: ["english-declarations", "where-clause", "respectively"],
      },
      {
        id: "roles",
        tags: ["role-prose", "role-conflict", "role-swap"],
      },
      {
        id: "constraints",
        tags: ["shape-explicit", "shape-mismatch"],
      },
      {
        id: "semantic-mutation",
        tags: ["hard-negative", "wrong-operator", "extra-term", "missing-role"],
      },
    ],
    foundationSuites: [],
    metamorphic: {
      casesPerLaw: 1,
      transforms: ["neutral-prose", "trailing-comment", "document-order"],
    },
    packs: [
      {
        capabilities: {
          "concept-vocabulary": { maturity: "probe", suiteIds: [] },
          "declarations-roles": probe,
          "shape-quantity-unit": probe,
          "law-recognition": probe,
          "diagnostics-refusal": probe,
          "project-macro": unsupported,
          "navigation-explanation": probe,
        },
        packId: pack.packId,
      },
    ],
    schemaVersion: 3,
    suites: [
      {
        id: suiteId,
        kind: "law",
        minimumPositiveCasesPerLaw: 5,
        minimumRefusalCasesPerLaw: 5,
        packId: pack.packId,
        path: "corpus.json",
        requiredDimensions: [
          "notation",
          "prose",
          "roles",
          "constraints",
          "semantic-mutation",
        ],
        requiredDiversity: {
          maximumProfileShare: 0.25,
          minimumDistinct: {
            mutationFamily: 5,
            projectTopology: 2,
            proseFamily: 5,
            semanticSkeleton: 5,
            syntaxStructure: 5,
          },
        },
        tier: "probe",
      },
    ],
    thresholds: {
      evidenceIntegrity: 100,
      lawPrecision: 99,
      lawRecall: 95,
      refusalPreservation: 100,
      roleAccuracy: 100,
    },
  };
  return { corpus, manifest };
}

export function compareScorecards(
  baseline: QualityScorecard,
  candidate: QualityScorecard,
): ScorecardComparison {
  const regressions: string[] = [];
  const improvements: string[] = [];
  const unchanged: string[] = [];
  const baselineLaws = new Map(
    baseline.laws.map((law) => [`${law.suiteId}/${law.lawId}`, law]),
  );
  const candidateLaws = new Map(
    candidate.laws.map((law) => [`${law.suiteId}/${law.lawId}`, law]),
  );
  for (const [id, before] of baselineLaws) {
    const after = candidateLaws.get(id);
    if (!after) {
      regressions.push(`${id}: missing from candidate`);
      continue;
    }
    for (const metric of [
      "recall",
      "precision",
      "roleAccuracy",
      "evidenceIntegrity",
      "refusalPreservation",
    ] as const) {
      const delta = after[metric].percent - before[metric].percent;
      const message = `${id}/${metric}: ${before[metric].percent.toFixed(1)}% -> ${after[metric].percent.toFixed(1)}%`;
      if (delta < 0) regressions.push(message);
      else if (delta > 0) improvements.push(message);
      else unchanged.push(message);
    }
  }
  for (const id of candidateLaws.keys()) {
    if (!baselineLaws.has(id)) improvements.push(`${id}: added measurement`);
  }
  return { improvements, regressions, unchanged };
}

export function findForbiddenRuntimeBranches(
  sources: readonly RuntimeSource[],
  forbiddenIds: readonly string[],
): RuntimeBranchViolation[] {
  const ids = [...new Set(forbiddenIds)].sort((left, right) => right.length - left.length);
  const violations: RuntimeBranchViolation[] = [];
  for (const file of sources) {
    if (/\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(file.path)) continue;
    let testOnly = false;
    for (const [index, sourceLine] of file.source.split(/\r?\n/u).entries()) {
      const line = sourceLine.trim();
      if (line === "#[cfg(test)]") testOnly = true;
      if (testOnly || line.startsWith("//") || line.startsWith("*")) continue;
      if (!/\b(?:if|else if|match|matches|switch|case)\b|=>|===?|!=|\.contains\(|\.ends_with\(/u.test(line)) {
        continue;
      }
      for (const id of ids) {
        if (quotedIdentifier(line, id)) {
          violations.push({
            id,
            line: index + 1,
            path: file.path,
            sourceLine: line,
          });
        }
      }
    }
  }
  return violations;
}

export function findCorpusTagProblems(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
): string[] {
  const declared = new Set(manifest.dimensions.flatMap((dimension) => dimension.tags));
  const used = new Set(
    [...corpora.values()].flatMap((corpus) =>
      corpus.cases.flatMap((item) => item.variationTags),
    ),
  );
  return [...declared]
    .filter((tag) => !used.has(tag))
    .map((tag) => `coverage tag ${tag} is not used by any corpus case`)
    .sort();
}

export function packagePackAssets(
  sources: PackAuthoringRequest["sources"],
  report: PackAuthoringReport,
): object {
  if (report.diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    throw new Error("cannot package a catalog with compiler errors");
  }
  return {
    compilerReport: report,
    packs: sources.map((source) => ({
      path: source.path,
      value: JSON.parse(source.source) as unknown,
    })),
    schemaVersion: 1,
  };
}

function scaffoldLawCases(law: AuthoringLaw): CorpusCase[] {
  const symbolSets = [
    ["y", "c", "x", "t", "z"],
    ["q", "k", "u", "s", "w"],
    ["r", "a", "b", "n", "p"],
    ["v", "h", "g", "j", "d"],
    ["o", "m", "i", "e", "f"],
  ] as const;
  const positives = symbolSets.map((symbols, index) => {
    const bindings = bindSymbols(law, symbols);
    const formula = renderFormula(law.semanticForms[index % law.semanticForms.length]!, bindings);
    return establishedCase(law, bindings, formula, index);
  });
  const negativeMutations = [
    { category: "wrong-operator", mutate: wrongOperator },
    { category: "extra-term", mutate: (formula: string) => `${formula}+z` },
    {
      category: "missing-role",
      mutate: (_formula: string, values: string[]) => `${values[0]}_{probe}=0`,
    },
    { category: "role-swap", mutate: swapFirstTwo },
    { category: "shape-mismatch", mutate: (formula: string) => formula },
  ] as const;
  const refusals = negativeMutations.map((mutation, index) => {
    const symbols = symbolSets[index]!;
    const bindings = bindSymbols(law, symbols);
    const formula = renderFormula(law.semanticForms[0]!, bindings);
    const mutated = mutation.mutate(formula, [...bindings.values()]);
    const shapeConflict = mutation.category === "shape-mismatch"
      ? ` The symbol $${symbols[0]}$ is explicitly a ${incompatibleShape(law.roles[0]?.shape)}.`
      : "";
    const prose = `${declaration(law, bindings, index)}${shapeConflict}`;
    const twoDocument = (index + 5) % 3 === 0;
    return {
      cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle: mutated },
      diversity: diversity(index + 5, mutation.category),
      documents: twoDocument
        ? [
            {
              content: `\\input{roles}\n$${mutated}$`,
              fileId: "main",
              path: "main.tex",
            },
            { content: prose, fileId: "roles", path: "roles.tex" },
          ]
        : [{
            content: `${prose}\n\n$${mutated}$`,
            fileId: "main",
            path: "main.md",
          }],
      expectation: "refused",
      id: `${law.id}-refusal-${index + 1}`,
      lawId: law.id,
      ...(twoDocument ? { mainFileId: "main" } : {}),
      refusalCategory: mutation.category,
      variationTags: [
        "hard-negative",
        "role-prose",
        mutation.category,
        ...(mutation.category === "role-swap" ? ["role-conflict"] : []),
      ],
    } satisfies CorpusCase;
  });
  return [...positives, ...refusals];
}

function establishedCase(
  law: AuthoringLaw,
  bindings: ReadonlyMap<string, string>,
  formula: string,
  index: number,
): CorpusCase {
  const wrappers = [
    `$${formula}$`,
    `\\[${formula}\\]`,
    `\\begin{equation}${formula}\\end{equation}`,
    `$ {${formula}} $`,
    `\\[\\left(${formula}\\right)\\]`,
  ];
  const prose = declaration(law, bindings, index);
  const twoDocument = index % 3 === 0;
  return {
    cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle: formula },
    diversity: diversity(index, "affirmative"),
    documents: twoDocument
      ? [
          {
            content: `\\input{roles}\n${wrappers[index]}`,
            fileId: "main",
            path: "main.tex",
          },
          { content: prose, fileId: "roles", path: "roles.tex" },
        ]
      : [{
          content: `${prose}\n\n${wrappers[index]}`,
          fileId: "main",
          path: index === 2 ? "main.tex" : "main.md",
        }],
    expectation: "established",
    expectedRoles: Object.fromEntries(bindings),
    id: `${law.id}-positive-${index + 1}`,
    lawId: law.id,
    ...(twoDocument ? { mainFileId: "main" } : {}),
    variationTags: [
      index === 1 ? "display-notation" : index === 3 ? "grouped" : "conventional-notation",
      index === 2 ? "respectively" : index === 4 ? "where-clause" : "english-declarations",
      "role-prose",
      "shape-explicit",
    ],
  };
}

function bindSymbols(
  law: AuthoringLaw,
  symbols: readonly string[],
): Map<string, string> {
  return new Map(law.roles.map((role, index) => [role.id, symbols[index] ?? `x_${index + 1}`]));
}

function renderFormula(source: string, bindings: ReadonlyMap<string, string>): string {
  let result = source;
  for (const [role, symbol] of [...bindings].sort(
    ([left], [right]) => right.length - left.length,
  )) {
    result = result.replace(new RegExp(escapeRegExp(role), "gu"), symbol);
  }
  return result;
}

function declaration(
  law: AuthoringLaw,
  bindings: ReadonlyMap<string, string>,
  index: number,
): string {
  const entries = [...bindings].map(
    ([role, symbol]) => `$${symbol}$ ${role.replaceAll("-", " ")}`,
  );
  const joined = englishList(entries);
  const variants = [
    `Let ${joined} denote the roles in ${law.title}.`,
    `In this model, ${joined} are used, respectively.`,
    `Suppose that ${joined} have their stated meanings.`,
    `Write ${joined}; each declaration is local to this relation.`,
    `Here ${joined}, with all roles explicitly declared.`,
  ];
  return variants[index % variants.length]!;
}

function diversity(index: number, mutationFamily: string) {
  const prose = ["let-series", "respectively", "suppose-that", "write-series", "where-clause"];
  const syntax = ["inline-math", "display-math", "equation-environment", "grouped-expression", "paired-delimiters"];
  return {
    batch: "authoring-probe",
    mutationFamily,
    projectTopology: index % 3 === 0 ? "two-document" : "single-document",
    proseFamily: prose[index % prose.length]!,
    semanticSkeleton: `${mutationFamily}-${index % 5}`,
    syntaxStructure: syntax[index % syntax.length]!,
  };
}

function wrongOperator(formula: string): string {
  if (formula.includes("=")) return formula.replace("=", "\\neq");
  if (formula.includes("\\cap")) return formula.replace("\\cap", "\\cup");
  if (formula.includes("\\cup")) return formula.replace("\\cup", "\\cap");
  return `${formula}+z`;
}

function swapFirstTwo(formula: string, values: string[]): string {
  const [first, second] = values;
  if (!first || !second) return `${formula}+z`;
  return formula
    .replaceAll(first, "__SEMATH_SWAP__")
    .replaceAll(second, first)
    .replaceAll("__SEMATH_SWAP__", second);
}

function incompatibleShape(shape: string | undefined): string {
  return shape === "scalar" ? "matrix" : "scalar";
}

function englishList(values: readonly string[]): string {
  if (values.length <= 1) return values[0] ?? "";
  if (values.length === 2) return `${values[0]} and ${values[1]}`;
  return `${values.slice(0, -1).join(", ")}, and ${values.at(-1)}`;
}

function quotedIdentifier(line: string, id: string): boolean {
  const escaped = escapeRegExp(id);
  return new RegExp(`["']${escaped}["']`, "u").test(line);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

export const AUTHORING_CAPABILITIES = CAPABILITY_IDS;
