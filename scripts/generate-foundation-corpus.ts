import { mkdir, readFile, writeFile } from "node:fs/promises";
import type { FoundationCase, FoundationCorpus } from "../packages/evaluation/src/index";

interface QuantityKind {
  aliases?: string[];
  defaultUnit?: string;
  dimension: { base: string; denominator: number; numerator: number }[];
  id: string;
  title: string;
}

interface Unit {
  aliases?: string[];
  id: string;
  symbol: string;
}

const root = new URL("../", import.meta.url);
const check = process.argv.includes("--check");
const pack = JSON.parse(
  await readFile(new URL("packs/quantities-units/v1.json", root), "utf8"),
) as { quantityKinds: QuantityKind[]; units: Unit[] };
const units = new Map(pack.units.map((unit) => [`quantities-units:${unit.id}`, unit]));

const cases: FoundationCase[] = pack.quantityKinds.map((kind, index) => {
  const unit = kind.defaultUnit ? units.get(kind.defaultUnit) : undefined;
  const symbol = "Q";
  const description = kind.aliases?.[0] ?? kind.title.toLowerCase();
  const unitPhrase = unit ? ` in ${unit.aliases?.[0] ?? unit.symbol}` : "";
  return declarationCase(
    `quantity-kind-${kind.id}`,
    `Let $${symbol}$ be ${description}${unitPhrase}.`,
    symbol,
    {
      dimension: dimensionDisplay(kind.dimension),
      quantityKindId: `quantities-units:${kind.id}`,
      ...(unit ? { unitId: `quantities-units:${unit.id}` } : {}),
    },
    index,
  );
});

const variants = [
  ["force-alias", "let $F$ denote an applied force measured in newtons.", "F", "force", "newton"],
  ["velocity-alias", "we write $V$ for point velocity in metres per second.", "V", "velocity", "metre-per-second"],
  ["duration-alias", "let $T$ be elapsed time in seconds.", "T", "duration", "second"],
  ["current-alias", "let $I$ be branch current in amperes.", "I", "electric-current", "ampere"],
  ["voltage-alias", "here $U$ denotes potential difference in volts.", "U", "voltage", "volt"],
  ["resistance-alias", "define $R$ as resistor resistance in ohms.", "R", "resistance", "ohm"],
  ["acceleration-alias", "let $A$ represent an acceleration vector in metres per second squared.", "A", "acceleration", "metre-per-second-squared"],
  ["length-alias", "let $L$ denote length measured in metres.", "L", "length", "metre"],
  ["frequency-alias", "let $H$ denote frequency in hertz.", "H", "frequency", "hertz"],
] as const;
for (const [id, content, needle, kindId, unitId] of variants) {
  const kind = pack.quantityKinds.find((candidate) => candidate.id === kindId)!;
  cases.push(
    declarationCase(
      id,
      content,
      needle,
      {
        dimension: dimensionDisplay(kind.dimension),
        quantityKindId: `quantities-units:${kindId}`,
        unitId: `quantities-units:${unitId}`,
      },
      cases.length,
    ),
  );
}

for (const [id, content, needle, kindId] of [
  ["area-paraphrase", "let $S$ be cross-sectional area.", "S", "area"],
  ["temperature-paraphrase", "let $T$ be thermodynamic temperature in kelvin.", "T", "temperature"],
] as const) {
  const kind = pack.quantityKinds.find((candidate) => candidate.id === kindId)!;
  cases.push(
    declarationCase(
      id,
      content,
      needle,
      {
        dimension: dimensionDisplay(kind.dimension),
        quantityKindId: `quantities-units:${kindId}`,
        ...(kind.defaultUnit ? { unitId: kind.defaultUnit } : {}),
      },
      cases.length,
    ),
  );
}

for (const [id, content, needle, code] of [
  ["mass-in-seconds", "Let $m$ be mass in seconds.", "$m$", "quantity-unit-dimension-mismatch"],
  ["duration-in-kilograms", "Let $t$ be duration in kilograms.", "$t$", "quantity-unit-dimension-mismatch"],
  ["velocity-in-newtons", "Let $v$ be velocity in newtons.", "$v$", "quantity-unit-dimension-mismatch"],
  ["force-plus-acceleration", "Let $F$ be force, $m$ mass, and $a$ acceleration. $F=m+a$", "F=m+a", "quantity-addition-dimension-mismatch"],
  ["velocity-from-mass-time", "Let $v$ be velocity, $m$ mass, and $t$ duration. $v=m/t$", "v=m/t", "quantity-assignment-dimension-mismatch"],
  ["force-from-mass-time", "Let $F$ be force, $m$ mass, and $t$ duration. $F=m/t$", "F=m/t", "quantity-assignment-dimension-mismatch"],
] as const) {
  cases.push({
    cursor: { edge: "after", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: "main.md" }],
    expectation: { diagnosticCode: code },
    id,
    variationTags: ["dimension-mismatch", "hard-negative", "semantic-mutation", "unit-conflict", "wrong-role"],
  });
}

for (const [id, content, needle, symbol, dimension] of [
  ["velocity-propagation", "Let $d$ be length and $t$ duration. $v=d/t$. The derived value is $v$.", "v$", "v", "length · time^-1"],
  ["force-propagation", "Let $m$ be mass and $a$ acceleration. $F=m*a$. The derived value is $F$.", "F$", "F", "length · mass · time^-2"],
  ["power-propagation", "Let $F$ be force and $v$ velocity. $P=F\\cdot v$. The derived value is $P$.", "P$", "P", "length^2 · mass · time^-3"],
  ["current-propagation", "Let $q$ be electric charge and $t$ duration. $I=q/t$. The derived value is $I$.", "I$", "I", "electric-current"],
  ["alias-propagation", "Let $v$ be velocity. $u=v$. The derived value is $u$.", "u$", "u", "length · time^-1"],
  ["addition-propagation", "Let $x$ be length. Let $y$ be length. $s=x+y$. The derived value is $s$.", "s$", "s", "length"],
] as const) {
  cases.push({
    cursor: { edge: "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: "main.md" }],
    expectation: { dimension, symbol },
    id,
    variationTags: [
      "dimensions",
      "english-declarations",
      "role-prose",
      "same-dimension",
      "typed",
    ],
  });
}

if (cases.length !== 46) throw new Error(`expected 46 foundation cases, got ${cases.length}`);
const corpus: FoundationCorpus = {
  cases,
  domain: "quantities-foundation",
  schemaVersion: 1,
};
const output = `${JSON.stringify(corpus, null, 2)}\n`;
const path = new URL("fixtures/foundation/quantities-units.json", root);
if (check) {
  if (await readFile(path, "utf8").catch(() => "") !== output) {
    throw new Error("fixtures/foundation/quantities-units.json: generated corpus is stale");
  }
} else {
  await mkdir(new URL("fixtures/foundation/", root), { recursive: true });
  await writeFile(path, output);
}
console.log(`${check ? "verified" : "generated"} quantities foundation corpus (${cases.length} cases)`);

const scientificCases: FoundationCase[] = [
  proseDefinition("let-scalar", "Let $x$ be a scalar.", "$x$", "x", "a scalar", "association", ["let", "single-symbol"]),
  proseDefinition("given-matrix", "Given $A$ as the system matrix.", "$A$", "A", "the system matrix", "association", ["given", "role-descriptions"]),
  proseDefinition("take-state", "Take $z$ to be the latent state.", "$z$", "z", "the latent state", "association", ["take", "role-descriptions"]),
  proseDefinition("suppose-count", "Suppose $n$ is the sample count.", "$n$", "n", "the sample count", "association", ["suppose", "role-descriptions"]),
  proseDefinition("where-duration", "Where $t$ represents elapsed time.", "$t$", "t", "elapsed time", "association", ["where-clause", "declaration-after"]),
  proseDefinition("write-objective", "We write $f$ for the objective function.", "$f$", "f", "the objective function", "association", ["write", "conventional-notation"]),
  proseDefinition("define-probability", "Define $p$ as the empirical probability.", "$p$", "p", "the empirical probability", "association", ["define", "role-descriptions"]),
  proseDefinition("passive-input", "The control input is denoted by $u$.", "$u$", "u", "control input", "evidence", ["passive", "declaration-before"], "english-passive-definition"),
  proseDefinition("pair-respectively", "Let $a$ and $b$ denote the lower bound and the upper bound, respectively.", "$a$", "a", "lower bound", "association", ["respectively", "multi-symbol-declaration"]),
  proseDefinition("triple-in-order", "Let $x$, $y$, and $z$ represent the input, state, and output, in that order.", "$y$", "y", "state", "association", ["in-that-order", "multi-symbol-declaration"]),
  proseDefinition("quad-respectively", "Let $a$, $b$, $c$, and $d$ denote gain, bias, scale, and offset, respectively.", "$d$", "d", "offset", "association", ["respectively", "plural-declaration"]),
  proseDefinition("shared-vector-spaces", "Let $U$ and $V$ be vector spaces.", "$V$", "V", "vector spaces", "association", ["shared-description", "plural-declaration"]),
  proseDefinition("apposition", "$S$, the covariance matrix, is fixed.", "$S$", "S", "covariance matrix", "association", ["apposition", "declaration-after"]),
  proseDefinition("parenthetical", "The normalized vector ($v$) is observed.", "$v$", "v", "normalized vector", "association", ["parenthetical", "declaration-before"]),
  proseDefinition("notation-table", "| Symbol | Meaning |\n|---|---|\n| $r$ | residual norm |", "$r$", "r", "residual norm", "association", ["notation-table", "multiline-prose"]),
  proseAssumption("positive", "Assume $m$ is strictly positive.", "$m$", "m", "sign", "strictly-positive", ["positivity", "assume"]),
  proseAssumption("symmetric", "Suppose $A$ is symmetric.", "$A$", "A", "structure", "symmetric", ["symmetry", "suppose"]),
  proseAssumption("positive-definite", "Assume $H$ is positive definite.", "$H$", "H", "definiteness", "positive-definite", ["definiteness", "assume"]),
  proseAssumption("continuous", "Let $f$ be continuous on the domain.", "$f$", "f", "regularity", "continuous", ["continuity", "constraints"]),
  proseAssumption("differentiable", "Given $g$ differentiable near the optimum.", "$g$", "g", "regularity", "differentiable", ["differentiability", "constraints"]),
  proseAssumption("invertible", "Assume $J$ is invertible at the solution.", "$J$", "J", "algebraic-property", "invertible", ["invertibility", "constraints"]),
  proseAssumption("steady-state", "At steady state, let $x$ denote the operating point.", "$x$", "x", "regime", "steady-state", ["steady-state", "constraints"]),
  proseAssumption("small-signal", "Under small-signal operation, $u$ is the perturbation input.", "$u$", "u", "regime", "small-signal", ["small-signal", "constraints"]),
  proseRefusal("hypothetical", "If $B$ were invertible, the solve would be unique.", "$B$", "B", "invertible", ["counterfactual", "semantic-mutation"]),
  proseRefusal("hedged", "The symbol $C$ may be a continuous function.", "$C$", "C", "continuous", ["hedging", "semantic-mutation"]),
  proseRefusal("cited", "According to \\cite{prior}, $D$ is symmetric.", "$D$", "D", "symmetric", ["citation", "semantic-mutation"]),
  proseRefusal("negated", "The matrix $E$ is not positive definite.", "$E$", "E", "positive-definite", ["negation", "semantic-mutation"]),
  proseRefusal("alternative", "Alternatively, $q$ represents the heat flux.", "$q$", "q", undefined, ["alternative", "semantic-mutation"]),
  proseRefusal("commented", "% Assume $R$ is invertible.\nThe calculation continues.", "$R$", "R", "invertible", ["comment", "semantic-mutation"]),
  proseRefusal("arity-mismatch", "Let $i$, $j$, and $k$ denote row and column indices, respectively.", "$j$", "j", undefined, ["arity-mismatch", "respectively"]),
  {
    cursor: { edge: "before", fileId: "main", needle: "v$." },
    documents: [{ content: "Let $v$ be velocity. The measured signal is $v$.", fileId: "main", path: "main.tex" }],
    expectation: { quantityKindId: "quantities-units:velocity", symbol: "v" },
    id: "classify-velocity",
    metric: "classification",
    variationTags: ["english-declarations", "prose", "role-descriptions"],
  },
  {
    cursor: { edge: "before", fileId: "main", needle: "x$." },
    documents: [{ content: "Let $x$ be the input. The estimate is $x$.", fileId: "main", path: "main.tex" }],
    expectation: { definitionDescription: "the input", symbol: "x" },
    id: "scope-after-declaration",
    metric: "scope",
    variationTags: ["declarations-before", "project-scope", "prose"],
  },
  {
    cursor: { edge: "before", fileId: "main", needle: "y$." },
    documents: [{ content: "The estimate is $y$. Let $y$ be the output.", fileId: "main", path: "main.tex" }],
    expectation: { excludedDefinitionSymbol: "y" },
    id: "scope-refuses-future-declaration",
    metric: "scope",
    variationTags: ["declarations-after", "project-scope", "semantic-mutation"],
  },
  {
    cursor: { edge: "before", fileId: "main", needle: "s$." },
    documents: [
      { content: "Let $s$ be the system state.", fileId: "defs", path: "defs.tex" },
      { content: "\\input{defs}\nThe estimate is $s$.", fileId: "main", path: "main.tex" },
    ],
    expectation: { definitionDescription: "the system state", symbol: "s" },
    id: "scope-included-declaration",
    metric: "scope",
    variationTags: ["included-declarations", "multi-file", "project-context", "project-scope"],
  },
];

if (scientificCases.length !== 34) {
  throw new Error(`expected 34 scientific prose cases, got ${scientificCases.length}`);
}
const scientificCorpus: FoundationCorpus = {
  cases: scientificCases,
  domain: "scientific-prose-foundation",
  schemaVersion: 1,
};
const scientificOutput = `${JSON.stringify(scientificCorpus, null, 2)}\n`;
const scientificPath = new URL("fixtures/foundation/scientific-prose.json", root);
if (check) {
  if (await readFile(scientificPath, "utf8").catch(() => "") !== scientificOutput) {
    throw new Error("fixtures/foundation/scientific-prose.json: generated corpus is stale");
  }
} else {
  await writeFile(scientificPath, scientificOutput);
}
console.log(`${check ? "verified" : "generated"} scientific prose foundation corpus (${scientificCases.length} cases)`);

function declarationCase(
  id: string,
  content: string,
  needle: string,
  expectation: FoundationCase["expectation"],
  index: number,
): FoundationCase {
  return {
    cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: index % 3 ? "main.md" : "main.tex" }],
    expectation,
    id,
    variationTags: [
      "conventional-notation",
      "dimensions",
      "english-declarations",
      "role-prose",
      ...(expectation.unitId ? ["unit-context"] : []),
    ],
  };
}

function dimensionDisplay(
  dimension: readonly { base: string; denominator: number; numerator: number }[],
): string {
  if (!dimension.length) return "dimensionless";
  return [...dimension]
    .sort((left, right) => left.base.localeCompare(right.base))
    .map(({ base, denominator, numerator }) => {
      if (numerator === 1 && denominator === 1) return base;
      if (denominator === 1) return `${base}^${numerator}`;
      return `${base}^(${numerator}/${denominator})`;
    })
    .join(" · ");
}

function proseDefinition(
  id: string,
  content: string,
  needle: string,
  symbol: string,
  definitionDescription: string,
  metric: FoundationCase["metric"],
  tags: readonly string[],
  definitionEvidenceRuleId?: string,
): FoundationCase {
  return {
    cursor: { edge: "before", fileId: "main", needle: needle.replace(/^\$/u, "") },
    documents: [{ content, fileId: "main", path: "main.tex" }],
    expectation: {
      definitionDescription,
      ...(definitionEvidenceRuleId ? { definitionEvidenceRuleId } : {}),
      symbol,
    },
    id,
    ...(metric ? { metric } : {}),
    variationTags: [...new Set(["different-role-set", "english-declarations", "prose", ...tags])],
  };
}

function proseAssumption(
  id: string,
  content: string,
  needle: string,
  subject: string,
  assumptionKind: string,
  assumptionValue: string,
  tags: readonly string[],
): FoundationCase {
  return {
    cursor: { edge: "before", fileId: "main", needle: needle.replace(/^\$/u, "") },
    documents: [{ content, fileId: "main", path: "main.tex" }],
    expectation: { assumptionKind, assumptionSubject: subject, assumptionValue },
    id,
    metric: "assumption",
    variationTags: [...new Set(["dimensions", "english-declarations", "prose", "typed", ...tags])],
  };
}

function proseRefusal(
  id: string,
  content: string,
  needle: string,
  symbol: string,
  excludedAssumptionValue: string | undefined,
  tags: readonly string[],
): FoundationCase {
  return {
    cursor: { edge: "before", fileId: "main", needle: needle.replace(/^\$/u, "") },
    documents: [{ content, fileId: "main", path: "main.tex" }],
    expectation: {
      ...(excludedAssumptionValue ? { excludedAssumptionValue } : {}),
      excludedDefinitionSymbol: symbol,
    },
    id,
    metric: "refusal",
    variationTags: [...new Set(["hard-negative", "prose", "semantic-mutation", ...tags])],
  };
}
