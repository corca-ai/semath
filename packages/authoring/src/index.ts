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
  archetypes: readonly {
    adoptedLaws: readonly string[];
    archetypeId: string;
    matchingLaws: readonly string[];
    parameterSlots: readonly string[];
  }[];
  bridges: readonly {
    bridgeId: string;
    ownerPackId: string;
    sourceConceptId: string;
    targetConceptId: string;
  }[];
  collisions: readonly {
    distinguishingEvidence: readonly string[];
    leftRelationId: string;
    rightRelationId: string;
    structuralKey: string;
  }[];
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
  schemaVersion: 3;
  signatures: readonly {
    capabilities: readonly string[];
    dependencies: readonly string[];
    packId: string;
    packKind: "application" | "capability" | "field";
    packVersion: string;
    terms: readonly { source: string; text: string }[];
    structuralKeys: readonly string[];
    title: string;
  }[];
}

export interface PackAuthoringRequest {
  schemaVersion: 3;
  sources: readonly { path: string; source: string }[];
}

export interface AuthoringPack {
  concepts: readonly AuthoringConcept[];
  laws: readonly AuthoringLaw[];
  packId: string;
  title: string;
}

export interface AuthoringConcept {
  id: string;
  title: string;
}

export interface AuthoringLaw {
  activationPhrases: readonly string[];
  canonicalRelation: string;
  id: string;
  representations: readonly string[];
  roles: readonly { concept: string; description?: string; id: string; shape?: string }[];
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

export function projectValidatedPack(
  value: unknown,
  compiledForms: readonly PackCanonicalForm[] = [],
): AuthoringPack {
  if (!isRecord(value)) throw new Error("validated pack must be an object");
  const packId = requiredString(value.packId, "validated pack.packId");
  const title = requiredString(value.title, "validated pack.title");
  if (!Array.isArray(value.concepts)) {
    throw new Error("validated pack.concepts must be an array");
  }
  if (!Array.isArray(value.laws)) throw new Error("validated pack.laws must be an array");
  return {
    concepts: value.concepts.map((candidate, conceptIndex) => {
      const path = `validated pack.concepts[${conceptIndex}]`;
      if (!isRecord(candidate)) throw new Error(`${path} must be an object`);
      return {
        id: requiredString(candidate.id, `${path}.id`),
        title: requiredString(candidate.title, `${path}.title`),
      };
    }),
    laws: value.laws.map((candidate, lawIndex) => {
      const path = `validated pack.laws[${lawIndex}]`;
      if (!isRecord(candidate)) throw new Error(`${path} must be an object`);
      if (!Array.isArray(candidate.roles)) throw new Error(`${path}.roles must be an array`);
      if (candidate.activationPhrases !== undefined && !Array.isArray(candidate.activationPhrases)) {
        throw new Error(`${path}.activationPhrases must be an array`);
      }
      if (candidate.representations !== undefined && !Array.isArray(candidate.representations)) {
        throw new Error(`${path}.representations must be an array`);
      }
      return {
        activationPhrases: (candidate.activationPhrases ?? []).map((phrase, phraseIndex) =>
          requiredString(phrase, `${path}.activationPhrases[${phraseIndex}]`),
        ),
        canonicalRelation: requiredString(
          candidate.canonicalRelation ?? compiledForms.find(
            (form) => form.packId === packId && form.lawId === candidate.id && form.formIndex === 0,
          )?.source,
          `${path}.canonicalRelation`,
        ),
        id: requiredString(candidate.id, `${path}.id`),
        representations: (candidate.representations ?? []).map((form, formIndex) =>
          requiredString(form, `${path}.representations[${formIndex}]`),
        ),
        roles: candidate.roles.map((role, roleIndex) => {
          const rolePath = `${path}.roles[${roleIndex}]`;
          if (!isRecord(role)) throw new Error(`${rolePath} must be an object`);
          const description = optionalString(role.description, `${rolePath}.description`);
          const shape = optionalString(role.shape, `${rolePath}.shape`);
          return {
            concept: requiredString(role.concept, `${rolePath}.concept`),
            id: requiredString(role.id, `${rolePath}.id`),
            ...(description ? { description } : {}),
            ...(shape ? { shape } : {}),
          };
        }),
        title: requiredString(candidate.title, `${path}.title`),
      };
    }),
    packId,
    title,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requiredString(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path} must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown, path: string): string | undefined {
  if (value === undefined) return undefined;
  return requiredString(value, path);
}

export function scaffoldPackWorkspace(pack: AuthoringPack): PackWorkspaceScaffold {
  const suiteId = `${pack.packId}-probe`;
  const conceptTitles = new Map(
    pack.concepts.map((concept) => [`${pack.packId}:${concept.id}`, concept.title]),
  );
  const corpus: Corpus = {
    cases: pack.laws.flatMap((law) => scaffoldLawCases(law, conceptTitles)),
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
    materializedSuiteIds: [],
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
    schemaVersion: 4,
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
    if (isTestSourcePath(file.path)) continue;
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

function isTestSourcePath(path: string): boolean {
  const normalized = path.replaceAll("\\", "/");
  return /\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(normalized) ||
    /(?:^|\/)(?:tests?\.rs|[^/]+_tests?\.rs|tests?\/.*\.rs)$/u.test(normalized);
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
    schemaVersion: 2,
  };
}

function scaffoldLawCases(
  law: AuthoringLaw,
  conceptTitles: ReadonlyMap<string, string>,
): CorpusCase[] {
  const relations = [law.canonicalRelation, ...law.representations];
  const symbolSets = [
    ["y", "c", "x", "t", "z"],
    ["q", "k", "u", "s", "w"],
    ["r", "a", "b", "j", "p"],
    ["v", "h", "g", "j", "d"],
    ["o", "m", "i", "e", "f"],
  ] as const;
  const positives = symbolSets.map((symbols, index) => {
    const bindings = bindSymbols(law, symbols);
    const formula = renderFormula(relations[index % relations.length]!, bindings);
    return recognizedCase(law, bindings, formula, index, conceptTitles);
  });
  const negativeMutations = [
    { category: "wrong-operator", mutate: wrongOperator },
    { category: "extra-term", mutate: (formula: string) => `${formula}+z` },
    {
      category: "missing-role",
      mutate: (_formula: string, values: string[]) => `${values[0]}_{probe}=0`,
    },
    { category: "role-swap", mutate: (formula: string) => formula },
    { category: "shape-mismatch", mutate: (formula: string) => formula },
  ] as const;
  const refusals = negativeMutations.map((mutation, index) => {
    const symbols = symbolSets[index]!;
    const bindings = bindSymbols(law, symbols);
    const formula = renderFormula(law.canonicalRelation, bindings);
    const shapedRole = law.roles.find((role) => role.shape);
    const roleSwapIsMeaningful = law.roles.some((role, roleIndex) =>
      law.roles.slice(roleIndex + 1).some((other) => other.concept !== role.concept)
    );
    const effectiveCategory = mutation.category === "shape-mismatch" && !shapedRole
      ? "missing-role"
      : mutation.category === "role-swap" && !roleSwapIsMeaningful
        ? "role-domain-mismatch"
        : mutation.category;
    const mutated = effectiveCategory === "missing-role"
      ? `${[...bindings.values()][0]}_{probe}=0`
      : effectiveCategory === "role-domain-mismatch"
        ? replaceRoleWithNumber(formula, [...bindings.values()][0]!)
        : mutation.mutate(formula, [...bindings.values()]);
    const declarationBindings = effectiveCategory === "role-swap"
      ? conflictFirstTwoBindings(bindings)
      : bindings;
    const shapeOverrides = effectiveCategory === "shape-mismatch" && shapedRole
      ? new Map([[shapedRole.id, incompatibleShape(shapedRole.shape)]])
      : undefined;
    const prose = declaration(
      law,
      declarationBindings,
      index,
      conceptTitles,
      shapeOverrides,
    );
    const twoDocument = (index + 5) % 3 === 0;
    return {
      cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle: mutated },
      diversity: diversity(index + 5, effectiveCategory),
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
      refusalCategory: effectiveCategory,
      variationTags: [
        "hard-negative",
        "role-prose",
        effectiveCategory,
        ...(effectiveCategory === "role-swap" ? ["role-conflict"] : []),
      ],
    } satisfies CorpusCase;
  });
  return [...positives, ...refusals];
}

function recognizedCase(
  law: AuthoringLaw,
  bindings: ReadonlyMap<string, string>,
  formula: string,
  index: number,
  conceptTitles: ReadonlyMap<string, string>,
): CorpusCase {
  const wrappers = [
    `$${formula}$`,
    `\\[${formula}\\]`,
    `\\begin{equation}${formula}\\end{equation}`,
    `$ {${formula}} $`,
    `\\[\\left(${formula}\\right)\\]`,
  ];
  const prose = declaration(law, bindings, index, conceptTitles);
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
    expectation: "recognized",
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
    result = result.replace(
      new RegExp(`(?<![A-Za-z0-9])${escapeRegExp(role)}(?![A-Za-z0-9])`, "gu"),
      symbol,
    );
  }
  return result
    .replace(/\bVar(?=\s*\()/gu, "\\operatorname{Var}")
    .replace(/\bCov(?=\s*\()/gu, "\\operatorname{Cov}")
    .replace(/\bE(?=\s*\()/gu, "\\operatorname{E}")
    .replace(/\bP(?=\s*\()/gu, "\\mathbb{P}")
    .replace(/\blog\b/gu, "\\log");
}

function declaration(
  law: AuthoringLaw,
  bindings: ReadonlyMap<string, string>,
  index: number,
  conceptTitles: ReadonlyMap<string, string>,
  shapeOverrides?: ReadonlyMap<string, string>,
): string {
  const lawPhrase = law.activationPhrases[0] ?? law.title;
  const conceptCounts = new Map<string, number>();
  for (const role of law.roles) {
    conceptCounts.set(role.concept, (conceptCounts.get(role.concept) ?? 0) + 1);
  }
  const entries = [...bindings].map(([roleId, symbol]) => {
    const role = law.roles.find((candidate) => candidate.id === roleId);
    const repeatedConcept = role && (conceptCounts.get(role.concept) ?? 0) > 1;
    const words = ((repeatedConcept ? role.description : undefined) ??
      conceptTitles.get(role?.concept ?? "") ??
      role?.description ??
      roleId.replaceAll("-", " "))
      .trim()
      .replace(/[.!?]+$/u, "")
      .toLocaleLowerCase("en-US");
    const shape = shapeOverrides?.get(roleId) ?? role?.shape;
    const description = shape ? descriptionWithShape(words, shape) : words;
    return { description, symbol: `$${symbol}$` };
  });
  const clauses = entries.map(({ description, symbol }) => `${symbol} denotes ${description}`);
  const letClauses = entries.map(({ description, symbol }) => `${symbol} denote ${description}`);
  const predicates = entries.map(({ description, symbol }) => `${symbol} is ${description}`);
  const symbols = englishList(entries.map((entry) => entry.symbol));
  const descriptions = englishList(entries.map((entry) => entry.description));
  const variants = [
    `For ${lawPhrase}, let ${englishList(letClauses)}.`,
    `In ${lawPhrase}, let ${symbols} denote ${descriptions}, respectively.`,
    `For ${lawPhrase}, suppose ${englishList(predicates)}.`,
    `${entries.map(({ description, symbol }) => `We write ${symbol} for ${description}.`).join(" ")} This notation is used in ${lawPhrase}.`,
    `For ${lawPhrase}, here ${englishList(clauses)}.`,
  ];
  return variants[index % variants.length]!;
}

function descriptionWithShape(description: string, shape: string): string {
  if (shape === "vector") {
    return description.endsWith("vector")
      ? `n-dimensional ${description}`
      : `n-dimensional ${description} vector`;
  }
  if (shape === "matrix") {
    return description.endsWith("matrix")
      ? `n by n ${description}`
      : `n by n ${description} matrix`;
  }
  if (description.includes(shape)) return description;
  return `${description} ${shape}`;
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

function replaceRoleWithNumber(formula: string, symbol: string): string {
  return formula.replace(
    new RegExp(`(?<![A-Za-z0-9])${escapeRegExp(symbol)}(?![A-Za-z0-9])`, "u"),
    "0",
  );
}

function conflictFirstTwoBindings(
  bindings: ReadonlyMap<string, string>,
): ReadonlyMap<string, string> {
  const entries = [...bindings];
  if (entries.length < 2) return bindings;
  entries[0]![1] = entries[1]![1];
  return new Map(entries);
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
