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

if (cases.length !== 52) throw new Error(`expected 52 foundation cases, got ${cases.length}`);
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

scientificCases.push(...constructionDefinitionCases(), ...discourseMutationCases());

scientificCases.push(
  acronymDefinition(
    "acronym-long-short",
    "Expected calibration error (ECE) is reported.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}=0",
    "Expected calibration error",
    "english-long-short-parenthetical",
    ["acronym", "parenthetical", "named-operator"],
  ),
  acronymDefinition(
    "acronym-short-long",
    "RMSE (root mean squared error) is reported.\n$\\operatorname{RMSE}=0$.",
    "\\operatorname{RMSE}",
    "root mean squared error",
    "english-short-long-parenthetical",
    ["acronym", "parenthetical", "reverse-direction"],
    "RMSE",
  ),
  acronymDefinition(
    "acronym-stands-for",
    "ECE stands for expected calibration error.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    "expected calibration error",
    "english-short-defines-long",
    ["acronym", "stands-for", "active-voice"],
  ),
  acronymDefinition(
    "acronym-abbreviated-as",
    "Expected calibration error is abbreviated as ECE.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    "Expected calibration error",
    "english-long-defines-short",
    ["acronym", "abbreviated-as", "passive"],
  ),
  acronymDefinition(
    "acronym-hereafter",
    "Expected calibration error, hereafter ECE, is reported.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    "Expected calibration error",
    "english-long-defines-short",
    ["acronym", "hereafter", "apposition"],
  ),
  acronymRefusal(
    "acronym-negated",
    "ECE does not mean expected calibration error.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    ["acronym", "negation"],
  ),
  acronymRefusal(
    "acronym-hypothetical",
    "If ECE meant expected calibration error, continue.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    ["acronym", "counterfactual"],
  ),
  acronymRefusal(
    "acronym-hedged",
    "ECE might mean expected calibration error.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    ["acronym", "hedging"],
  ),
  acronymRefusal(
    "acronym-cited",
    "According to the reference, ECE means expected calibration error.\n$\\operatorname{ECE}=0$.",
    "\\operatorname{ECE}",
    ["acronym", "citation"],
  ),
  acronymRefusal(
    "acronym-quoted",
    'The phrase "ECE means expected calibration error" is quoted.\n$\\operatorname{ECE}=0$.',
    "\\operatorname{ECE}",
    ["acronym", "quotation"],
  ),
  acronymRefusal(
    "acronym-future-declaration",
    "$\\operatorname{ECE}=0$. Expected calibration error (ECE) is reported later.",
    "\\operatorname{ECE}",
    ["acronym", "declarations-after", "project-scope"],
  ),
  acronymDefinition(
    "acronym-resource",
    "\\newacronym{ece}{ECE}{expected calibration error}\n$\\operatorname{ECE}=0$",
    "\\operatorname{ECE}",
    "expected calibration error",
    "latex-acronym-declaration",
    ["acronym", "macro-provenance", "project-context"],
    "ECE",
    "main.tex",
  ),
  acronymDefinition(
    "glossary-resource",
    "\\newglossaryentry{ece}{name={ECE},description={expected calibration error}}\n$\\operatorname{ECE}=0$",
    "\\operatorname{ECE}",
    "expected calibration error",
    "latex-glossary-declaration",
    ["glossary", "macro-provenance", "project-context"],
    "ECE",
    "main.tex",
  ),
  acronymDefinition(
    "declared-math-operator",
    "\\DeclareMathOperator{\\ECE}{ECE}\n$\\ECE(x)=0$",
    "\\ECE(x)",
    "ECE",
    "latex-math-operator-declaration",
    ["macro-provenance", "named-operator", "project-context"],
    "ECE",
    "main.tex",
  ),
  acronymDefinition(
    "acronym-section-shadowing",
    "# Metrics\nExpected calibration error (ECE) is reported.\n$\\operatorname{ECE}=0$.\n# Engineering\nElectrical computer engineering (ECE) is discussed.\n$\\operatorname{ECE}=1$.",
    "\\operatorname{ECE}=1",
    "Electrical computer engineering",
    "english-long-short-parenthetical",
    ["acronym", "project-scope", "shadowing"],
  ),
  acronymRefusal(
    "acronym-plain-juxtaposition",
    "Expected calibration error (ECE) is reported. Plain $ECE$ stays three identifiers.",
    "ECE$",
    ["acronym", "plain-juxtaposition", "representation-boundary"],
  ),
);

validateScientificCases(scientificCases);
if (scientificCases.length !== 150) {
  throw new Error(`expected 150 scientific prose cases, got ${scientificCases.length}`);
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

function constructionDefinitionCases(): FoundationCase[] {
  const rows = [
    ["let-denote-state", "Let $x_1$ denote the hidden state.", "$x_1$", "x_1", "the hidden state", "active", "let-denote"],
    ["let-represent-signal", "Let $s_1$ represent the sampled signal.", "$s_1$", "s_1", "the sampled signal", "active", "let-represent"],
    ["where-denotes-loss", "Where $L_1$ denotes the training loss.", "$L_1$", "L_1", "the training loss", "trailing", "where-denote"],
    ["where-represents-rate", "Where $r_1$ represents the event rate.", "$r_1$", "r_1", "the event rate", "trailing", "where-represent"],
    ["take-vector", "Take $v_1$ to be the search direction.", "$v_1$", "v_1", "the search direction", "imperative", "take"],
    ["take-scalar", "Take $c_1$ as the regularization weight.", "$c_1$", "c_1", "the regularization weight", "imperative", "take-as"],
    ["given-mapping", "Given $T_1$ as the transition mapping.", "$T_1$", "T_1", "the transition mapping", "fronted", "given"],
    ["given-threshold", "Given $h_1$ to be the decision threshold.", "$h_1$", "h_1", "the decision threshold", "fronted", "given-to-be"],
    ["suppose-index", "Suppose $k_1$ is the active index.", "$k_1$", "k_1", "the active index", "assumption", "suppose-is"],
    ["assume-kernel", "Assume $K_1$ denotes the covariance kernel.", "$K_1$", "K_1", "the covariance kernel", "assumption", "assume-denote"],
    ["write-estimator", "We write $e_1$ for the calibrated estimator.", "$e_1$", "e_1", "the calibrated estimator", "convention", "write-for"],
    ["write-residual", "We write $q_1$ for the normalized residual.", "$q_1$", "q_1", "the normalized residual", "convention", "write-for"],
    ["define-score", "Define $a_1$ as the aggregate score.", "$a_1$", "a_1", "the aggregate score", "imperative", "define-as"],
    ["define-risk", "Define $R_1$ as the empirical risk.", "$R_1$", "R_1", "the empirical risk", "imperative", "define-as"],
    ["denote-distance", "Denote by $d_1$ the geodesic distance.", "$d_1$", "d_1", "geodesic distance", "inverted", "denote-by"],
    ["denote-measure", "Denote by $m_1$ the reference measure.", "$m_1$", "m_1", "reference measure", "inverted", "denote-by"],
    ["set-radius", "Set $b_1$ equal to the trust-region radius.", "$b_1$", "b_1", "the trust-region radius", "imperative", "set-equal"],
    ["set-budget", "Set $B_1$ equal to the iteration budget.", "$B_1$", "B_1", "the iteration budget", "imperative", "set-equal"],
    ["use-identity", "We use $I_1$ to represent the identity operator.", "$I_1$", "I_1", "the identity operator", "active", "use-represent"],
    ["use-mask", "We use $M_1$ to denote the observation mask.", "$M_1$", "M_1", "the observation mask", "active", "use-denote"],
    ["call-domain", "Call $D_1$ the feasible domain.", "$D_1$", "D_1", "feasible domain", "imperative", "call"],
    ["call-partition", "Call $P_1$ the validation partition.", "$P_1$", "P_1", "validation partition", "imperative", "call"],
    ["here-operator", "Here $A_1$ designates the averaging operator.", "$A_1$", "A_1", "the averaging operator", "contextual", "designate"],
    ["here-output", "Here $y_1$ denotes the predicted output.", "$y_1$", "y_1", "the predicted output", "contextual", "here-denote"],
    ["with-count", "With $n_1$ denoting the batch count, training continues.", "$n_1$", "n_1", "the batch count", "contextual", "with-denoting"],
    ["with-scale", "With $g_1$ representing the scale factor, the update follows.", "$g_1$", "g_1", "the scale factor", "contextual", "with-representing"],
    ["symbol-graph", "The symbol $G_1$ stands for the dependency graph.", "$G_1$", "G_1", "the dependency graph", "relational", "stands-for"],
    ["notation-map", "The notation $F_1$ refers to the feature map.", "$F_1$", "F_1", "the feature map", "relational", "refers-to"],
    ["passive-control", "The control sequence is denoted by $u_1$.", "$u_1$", "u_1", "control sequence", "passive", "denoted-by"],
    ["passive-representation", "The learned representation is represented by $z_1$.", "$z_1$", "z_1", "learned representation", "passive", "represented-by"],
    ["appositive-jacobian", "$J_1$, the local Jacobian, is evaluated once.", "$J_1$", "J_1", "local Jacobian", "apposition", "appositive"],
    ["appositive-prior", "$p_1$, the reference prior, remains fixed.", "$p_1$", "p_1", "reference prior", "apposition", "appositive"],
    ["parenthetical-feature", "The encoded feature ($f_1$) is cached.", "$f_1$", "f_1", "encoded feature", "parenthetical", "parenthetical"],
    ["parenthetical-target", "The observed target ($t_1$) is centered.", "$t_1$", "t_1", "observed target", "parenthetical", "parenthetical"],
    ["quantified-sample", "For each sample $w_1$, the score is recorded.", "$w_1$", "w_1", "sample", "quantified", "for-each"],
    ["quantified-coordinate", "For every coordinate $j_1$, the derivative exists.", "$j_1$", "j_1", "coordinate", "quantified", "for-every"],
    ["pair-order", "Let $l_1$ and $u_2$ denote the lower limit and the upper limit, respectively.", "$u_2$", "u_2", "upper limit", "coordination", "arity-2"],
    ["triple-order", "Let $i_1$, $s_2$, and $o_3$ represent the input, state, and output, in that order.", "$s_2$", "s_2", "state", "coordination", "arity-3"],
    ["quad-order", "Let $g_2$, $b_2$, $c_2$, and $r_2$ denote gain, bias, scale, and rate, respectively.", "$c_2$", "c_2", "scale", "coordination", "arity-4"],
    ["shared-spaces", "Let $U_2$, $V_2$, and $W_2$ be function spaces.", "$W_2$", "W_2", "function spaces", "coordination", "shared"],
  ] as const;
  return rows.map(([id, content, needle, symbol, description, voice, construction]) =>
    proseDefinition(
      `construction-${id}`,
      content,
      needle,
      symbol,
      description,
      "association",
      ["construction-corpus", voice, construction],
    ));
}

function discourseMutationCases(): FoundationCase[] {
  const properties = [
    ["symmetric", "symmetric"],
    ["positive-definite", "positive definite"],
    ["positive-semidefinite", "positive semidefinite"],
    ["negative-definite", "negative definite"],
    ["continuous", "continuous"],
    ["differentiable", "differentiable"],
    ["invertible", "invertible"],
    ["independent", "independent"],
    ["nonnegative", "nonnegative"],
    ["strictly-positive", "strictly positive"],
    ["steady-state", "steady state"],
    ["small-signal", "small-signal"],
    ["time-invariant", "time invariant"],
    ["idealized", "idealized"],
    ["positive", "positive"],
  ] as const;
  return properties.flatMap(([id, phrase], index) => {
    const citation = `study${index + 1}`;
    return [
      proseRefusal(`frame-${id}-conditional`, `If $A_${index}$ were ${phrase}, the argument would continue.`, `$A_${index}$`, `A_${index}`, phrase, ["conditionality", "minimal-feature-mutation"]),
      proseRefusal(`frame-${id}-hedged`, `$B_${index}$ might be ${phrase}.`, `$B_${index}$`, `B_${index}`, phrase, ["hedging", "minimal-feature-mutation"]),
      proseRefusal(`frame-${id}-negative`, `$C_${index}$ is not ${phrase}.`, `$C_${index}$`, `C_${index}`, phrase, ["negation", "minimal-feature-mutation"]),
      proseRefusal(`frame-${id}-cited`, `Earlier analysis \\parencite{${citation}} reports that $D_${index}$ is ${phrase}.`, `$D_${index}$`, `D_${index}`, phrase, ["citation", "minimal-feature-mutation"]),
    ];
  });
}

function validateScientificCases(cases: readonly FoundationCase[]): void {
  const ids = new Set<string>();
  const sentences = new Set<string>();
  for (const item of cases) {
    if (ids.has(item.id)) throw new Error(`duplicate scientific prose id: ${item.id}`);
    ids.add(item.id);
    const normalized = item.documents
      .map((document) => document.content.toLowerCase().replace(/\s+/gu, " ").trim())
      .join("\n");
    if (sentences.has(normalized)) {
      throw new Error(`duplicate normalized scientific prose sentence: ${item.id}`);
    }
    sentences.add(normalized);
  }
  const constructionCases = cases.filter((item) => item.variationTags.includes("construction-corpus"));
  const frameCases = cases.filter((item) => item.id.startsWith("frame-"));
  if (constructionCases.length !== 40 || frameCases.length !== 60) {
    throw new Error(`scientific prose diversity drift: constructions=${constructionCases.length} frames=${frameCases.length}`);
  }
}

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

function acronymDefinition(
  id: string,
  content: string,
  needle: string,
  definitionDescription: string,
  definitionEvidenceRuleId: string,
  tags: readonly string[],
  symbol = "ECE",
  path = "main.md",
): FoundationCase {
  return {
    cursor: { edge: "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path }],
    expectation: { definitionDescription, definitionEvidenceRuleId, symbol },
    id,
    metric: "association",
    variationTags: [
      "english-declarations",
      "notation",
      "prose",
      ...tags,
    ],
  };
}

function acronymRefusal(
  id: string,
  content: string,
  needle: string,
  tags: readonly string[],
): FoundationCase {
  return {
    cursor: { edge: "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: "main.md" }],
    expectation: { excludedDefinitionSymbol: "ECE" },
    id,
    metric: "refusal",
    variationTags: ["hard-negative", "prose", "semantic-mutation", ...tags],
  };
}
