import { readFile, readdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import {
  assertDomainPackResults,
  buildDomainPackFixture,
} from "./v0.11-domain-fixture.mjs";
import {
  assertActionPatternResults,
  buildActionPatternFixture,
} from "./v0.12-action-fixture.mjs";
import {
  assertRealisticProjectResults,
  buildRealisticProjectFixture,
} from "./v0.12-realistic-project-fixture.mjs";
import {
  assertScientificResults,
  buildScientificFixture,
} from "./v0.14-scientific-fixture.ts";
import {
  assertSyntheticProseResults,
  buildSyntheticProseFixture,
  parseSyntheticProseCorpus,
} from "./synthetic-prose-corpus.ts";
import {
  assertSyntheticFormulaResults,
  buildSyntheticFormulaFixture,
  parseSyntheticDomainCorpus,
} from "./synthetic-corpus.ts";
import {
  aggregateSemanticQuality,
  evaluateSemanticQualityBudgets,
} from "./semantic-quality.ts";

const productQuality = [];

const fixtureSets = [
  {
    fixtureUrl: new URL("../fixtures/v0.1/explicit-definition.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.1/explicit-definition.golden.json", import.meta.url),
    version: "v0.1",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.2/bound-variable-rename.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.2/bound-variable-rename.golden.json", import.meta.url),
    version: "v0.2",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.3/shape-diagnostics.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.3/shape-diagnostics.golden.json", import.meta.url),
    version: "v0.3",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.4/formula-intelligence.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.4/formula-intelligence.golden.json", import.meta.url),
    version: "v0.4",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/symbol-inspection.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/symbol-inspection.golden.json", import.meta.url),
    version: "v0.5",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/prose-shape-claims.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/prose-shape-claims.golden.json", import.meta.url),
    version: "v0.5-prose",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/scoped-domain-evidence.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/scoped-domain-evidence.golden.json", import.meta.url),
    version: "v0.5-domains",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/notation-consistency.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/notation-consistency.golden.json", import.meta.url),
    version: "v0.5-consistency",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/definition-hygiene.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/definition-hygiene.golden.json", import.meta.url),
    version: "v0.5-hygiene",
  },
  {
    fixtureUrl: new URL(
      "../fixtures/v0.6/probability-formula-intelligence.json",
      import.meta.url,
    ),
    goldenUrl: new URL(
      "../fixtures/v0.6/probability-formula-intelligence.golden.json",
      import.meta.url,
    ),
    version: "v0.6-probability",
  },
  {
    fixtureUrl: new URL(
      "../fixtures/v0.7/formula-rewrites.json",
      import.meta.url,
    ),
    goldenUrl: new URL(
      "../fixtures/v0.7/formula-rewrites.golden.json",
      import.meta.url,
    ),
    version: "v0.7-rewrites",
  },
  {
    fixtureUrl: new URL(
      "../fixtures/v0.8/semantic-inspection.json",
      import.meta.url,
    ),
    goldenUrl: new URL(
      "../fixtures/v0.8/semantic-inspection.golden.json",
      import.meta.url,
    ),
    version: "v0.8-inspection",
  },
  {
    fixtureUrl: new URL(
      "../fixtures/v0.10/reliable-project-semantics.json",
      import.meta.url,
    ),
    goldenUrl: new URL(
      "../fixtures/v0.10/reliable-project-semantics.golden.json",
      import.meta.url,
    ),
    version: "v0.10-reliable-project-semantics",
  },
  {
    fixtureUrl: new URL(
      "../fixtures/v0.11/action-capable-patterns.json",
      import.meta.url,
    ),
    goldenUrl: new URL(
      "../fixtures/v0.11/action-capable-patterns.golden.json",
      import.meta.url,
    ),
    version: "v0.11-action-capable-patterns",
  },
];

const build = spawnSync("cargo", ["build", "--locked", "-p", "semath-native"], {
  encoding: "utf8",
});
if (build.status !== 0) throw new Error(build.stderr || "native build failed");
await init({
  module_or_path: await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)),
});
const encoder = new TextEncoder();
const decoder = new TextDecoder();

for (const { fixtureUrl, goldenUrl, version } of fixtureSets) {
  const fixtureText = await readFile(fixtureUrl, "utf8");
  const fixture = JSON.parse(fixtureText);
  const native = spawnSync("./target/debug/semath-native", [], {
    encoding: "utf8",
    input: fixtureText,
  });
  if (native.status !== 0) throw new Error(native.stderr || "native fixture failed");
  const nativeResults = JSON.parse(native.stdout);

  const engine = new SemathEngine();
  engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
  const wasmResults = fixture.queries.map((query) =>
    JSON.parse(decoder.decode(engine.query(encoder.encode(JSON.stringify(query))))),
  );
  engine.free();

  const nativeJson = JSON.stringify(nativeResults);
  const wasmJson = JSON.stringify(wasmResults);
  if (nativeJson !== wasmJson) {
    throw new Error(`native/WASM semantic result mismatch for ${version}`);
  }
  const golden = JSON.parse(await readFile(goldenUrl, "utf8"));
  if (nativeJson !== JSON.stringify(golden.results)) {
    throw new Error(`semantic result differs from the ${version} golden fixture`);
  }
  recordProductQuality(fixture.queries, version);
  console.log(`parity OK: ${fixture.queries.length} ${version} queries`);
}

const corpus = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.11/domain-pack-recognition-corpus.json", import.meta.url),
    "utf8",
  ),
);
const { fixture, expectations } = buildDomainPackFixture(corpus);
const fixtureText = JSON.stringify(fixture);
const native = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: fixtureText,
  maxBuffer: 64 * 1024 * 1024,
});
if (native.status !== 0) throw new Error(native.stderr || "v0.11 native fixture failed");
const nativeResults = JSON.parse(native.stdout);
const engine = new SemathEngine();
engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
const wasmResults = fixture.queries.map((query) =>
  JSON.parse(decoder.decode(engine.query(encoder.encode(JSON.stringify(query))))),
);
engine.free();
if (JSON.stringify(nativeResults) !== JSON.stringify(wasmResults)) {
  throw new Error("native/WASM semantic result mismatch for v0.11 domain packs");
}
const summary = assertDomainPackResults(nativeResults, expectations);
recordProductQuality(fixture.queries, "v0.11-domain-packs");
console.log(
  `parity OK: ${summary.recognized} v0.11 patterns, ${summary.results} safety queries`,
);

const actionCorpus = JSON.parse(
  await readFile(
    new URL(
      "../fixtures/v0.12/action-pattern-calibration.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const actionFixture = buildActionPatternFixture(actionCorpus);
const actionFixtureText = JSON.stringify(actionFixture.fixture);
const nativeAction = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: actionFixtureText,
});
if (nativeAction.status !== 0) {
  throw new Error(nativeAction.stderr || "v0.12 native action fixture failed");
}
const nativeActionResults = JSON.parse(nativeAction.stdout);
const actionEngine = new SemathEngine();
actionEngine.resetProject(
  encoder.encode(JSON.stringify(actionFixture.fixture.snapshot)),
);
const wasmActionResults = actionFixture.fixture.queries.map((query) =>
  JSON.parse(
    decoder.decode(actionEngine.query(encoder.encode(JSON.stringify(query)))),
  ),
);
actionEngine.free();
if (JSON.stringify(nativeActionResults) !== JSON.stringify(wasmActionResults)) {
  throw new Error("native/WASM semantic result mismatch for v0.12 action patterns");
}
const actionSummary = assertActionPatternResults(
  nativeActionResults,
  actionFixture.expectations,
);
recordProductQuality(actionFixture.fixture.queries, "v0.12-actions");
console.log(
  `parity OK: ${actionSummary.recognized} action patterns, ${actionSummary.results} v0.12 surface/adversarial queries`,
);

const realisticCorpus = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.12/realistic-mixed-project.json", import.meta.url),
    "utf8",
  ),
);
const realistic = buildRealisticProjectFixture(realisticCorpus);
const realisticText = JSON.stringify(realistic.fixture);
const nativeRealistic = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: realisticText,
});
if (nativeRealistic.status !== 0) {
  throw new Error(
    nativeRealistic.stderr || "v0.12 native realistic project fixture failed",
  );
}
const nativeRealisticResults = JSON.parse(nativeRealistic.stdout);
const realisticEngine = new SemathEngine();
realisticEngine.resetProject(
  encoder.encode(JSON.stringify(realistic.fixture.snapshot)),
);
const wasmRealisticResults = realistic.fixture.queries.map((query) =>
  JSON.parse(
    decoder.decode(realisticEngine.query(encoder.encode(JSON.stringify(query)))),
  ),
);
realisticEngine.free();
if (
  JSON.stringify(nativeRealisticResults) !==
  JSON.stringify(wasmRealisticResults)
) {
  throw new Error("native/WASM semantic result mismatch for v0.12 realistic project");
}
const realisticSummary = assertRealisticProjectResults(
  nativeRealisticResults,
  realistic.expectations,
);
recordProductQuality(realistic.fixture.queries, "v0.12-realistic-project");
console.log(
  `parity OK: ${realisticSummary.results} v0.12 realistic mixed-project queries`,
);

const scientificCorpus = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.14/scientific-foundation.json", import.meta.url),
    "utf8",
  ),
);
const scientific = buildScientificFixture(scientificCorpus);
const scientificText = JSON.stringify(scientific.fixture);
const nativeScientific = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: scientificText,
});
if (nativeScientific.status !== 0) {
  throw new Error(nativeScientific.stderr || "v0.14 native scientific fixture failed");
}
const nativeScientificResults = JSON.parse(nativeScientific.stdout);
const scientificEngine = new SemathEngine();
scientificEngine.resetProject(
  encoder.encode(JSON.stringify(scientific.fixture.snapshot)),
);
const wasmScientificResults = scientific.fixture.queries.map((query) =>
  JSON.parse(
    decoder.decode(scientificEngine.query(encoder.encode(JSON.stringify(query)))),
  ),
);
scientificEngine.free();
if (
  JSON.stringify(nativeScientificResults) !==
  JSON.stringify(wasmScientificResults)
) {
  throw new Error("native/WASM semantic result mismatch for v0.14 scientific foundation");
}
const scientificSummary = assertScientificResults(
  nativeScientificResults,
  scientific.expectations,
);
recordProductQuality(scientific.fixture.queries, "v0.14-scientific");
console.log(
  `parity OK: ${scientificSummary.queries} v0.14 scientific foundation queries`,
);

const proseRoot = new URL("../fixtures/synthetic/v1/prose/", import.meta.url);
const proseNames = (await readdir(proseRoot))
  .filter((name) => name.endsWith(".json"))
  .sort();
const proseCorpora = await Promise.all(
  proseNames.map(async (name) =>
    parseSyntheticProseCorpus(
      JSON.parse(await readFile(new URL(name, proseRoot), "utf8")),
      name,
    ),
  ),
);
const prose = buildSyntheticProseFixture(proseCorpora);
const proseText = JSON.stringify(prose.fixture);
const nativeProse = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: proseText,
  maxBuffer: 64 * 1024 * 1024,
});
if (nativeProse.status !== 0) {
  throw new Error(nativeProse.stderr || "synthetic prose native fixture failed");
}
const nativeProseResults = JSON.parse(nativeProse.stdout);
const proseEngine = new SemathEngine();
proseEngine.resetProject(encoder.encode(JSON.stringify(prose.fixture.snapshot)));
const wasmProseResults = prose.fixture.queries.map((query) =>
  JSON.parse(
    decoder.decode(proseEngine.query(encoder.encode(JSON.stringify(query)))),
  ),
);
proseEngine.free();
if (JSON.stringify(nativeProseResults) !== JSON.stringify(wasmProseResults)) {
  throw new Error("native/WASM semantic result mismatch for synthetic prose");
}
const proseSummary = assertSyntheticProseResults(
  nativeProseResults,
  prose.expectations,
);
recordProductQuality(prose.fixture.queries, "synthetic-prose");
console.log(
  `parity OK: ${proseSummary.cases} synthetic prose queries (${proseSummary.supportedCoverageTargets}/${proseSummary.coverageTargets} coverage targets supported)`,
);

const formulaRoot = new URL("../fixtures/synthetic/v1/", import.meta.url);
const formulaNames = (await readdir(formulaRoot))
  .filter((name) => name.endsWith(".json"))
  .sort();
const formulaCorpora = await Promise.all(
  formulaNames.map(async (name) =>
    parseSyntheticDomainCorpus(
      JSON.parse(await readFile(new URL(name, formulaRoot), "utf8")),
      name,
    ),
  ),
);
const formula = buildSyntheticFormulaFixture(formulaCorpora);
const formulaText = JSON.stringify(formula.fixture);
const nativeFormula = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: formulaText,
  maxBuffer: 64 * 1024 * 1024,
});
if (nativeFormula.status !== 0) {
  throw new Error(nativeFormula.stderr || "synthetic formula native fixture failed");
}
const nativeFormulaResults = JSON.parse(nativeFormula.stdout);
const formulaEngine = new SemathEngine();
formulaEngine.resetProject(
  encoder.encode(JSON.stringify(formula.fixture.snapshot)),
);
const wasmFormulaResults = formula.fixture.queries.map((query) =>
  JSON.parse(
    decoder.decode(formulaEngine.query(encoder.encode(JSON.stringify(query)))),
  ),
);
formulaEngine.free();
if (JSON.stringify(nativeFormulaResults) !== JSON.stringify(wasmFormulaResults)) {
  throw new Error("native/WASM semantic result mismatch for synthetic formulas");
}
const formulaSummary = assertSyntheticFormulaResults(
  nativeFormulaResults,
  formula.expectations,
);
recordProductQuality(formula.fixture.queries, "synthetic-formulas");
console.log(
  `parity OK: ${formula.expectations.length} synthetic formula queries across ${formulaSummary.length} domains`,
);

const qualityBudgetFile = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.15/semantic-quality-budgets.json", import.meta.url),
    "utf8",
  ),
);
const productBudgets = qualityBudgetFile.budgets.filter(
  (budget) => budget.selector.field === "product",
);
const productScores = aggregateSemanticQuality(productQuality, ["capability"]);
for (const score of productScores) {
  console.log(
    `quality guardrail: ${score.capability} cases=${score.cases} exact=${score.caseAccuracyPercent}% known-false=${score.unexpectedItems}`,
  );
}
const productBudgetResults = evaluateSemanticQualityBudgets(
  productQuality,
  productBudgets,
);
const productViolations = productBudgetResults.flatMap((result) =>
  result.violations.map((violation) => `${result.budgetId}: ${violation}`),
);
if (productViolations.length > 0) {
  throw new Error(`product quality budget regression:\n${productViolations.join("\n")}`);
}
console.log(`product quality budgets OK: ${productBudgetResults.length} zero-defect/parity gates`);

function recordProductQuality(queries, suite) {
  for (const envelope of queries) {
    productQuality.push(observation("native-wasm-parity", suite));
    const capability = productCapability(envelope.query.kind);
    if (capability) productQuality.push(observation(capability, suite));
  }
}

function observation(capability, suite) {
  return {
    field: "product",
    domain: suite,
    topic: "exact-regression",
    capability,
    cases: 1,
    exactCases: 1,
    expectedItems: 0,
    matchedItems: 0,
    actualItems: 0,
    unexpectedItems: 0,
  };
}

function productCapability(kind) {
  if (kind === "definition") return "definitions";
  if (kind === "references") return "references";
  if (kind === "diagnostics" || kind === "explainDiagnostic") {
    return "diagnostics";
  }
  if (
    kind === "formulaCompletion" ||
    kind === "formulaRewrite" ||
    kind === "rename" ||
    kind === "prepareRename"
  ) {
    return "edits";
  }
  return undefined;
}
