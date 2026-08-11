import { readFile, readdir, writeFile } from "node:fs/promises";
import {
  generateLawDiversityCorpus,
  parseSyntheticDiversitySpec,
  type Corpus,
} from "../packages/evaluation/src/index";
import {
  type PackAuthoringReport,
  projectValidatedPack,
  scaffoldPackWorkspace,
} from "../packages/authoring/src/index";

interface EngineeringSuite {
  id: string;
  lawIds?: readonly string[];
  packPath: string;
}

const root = new URL("../", import.meta.url);
const check = process.argv.includes("--check");
const spec = parseSyntheticDiversitySpec(JSON.parse(
  await readFile(new URL("fixtures/synthetic-diversity-spec.json", root), "utf8"),
));
const suites: readonly EngineeringSuite[] = [
  {
    id: "mechanics-engineering-depth",
    lawIds: ["linear-momentum-definition"],
    packPath: "packs/classical-mechanics/v1.json",
  },
  {
    id: "circuits-engineering-depth",
    lawIds: ["inductor-voltage-law"],
    packPath: "packs/circuits/v1.json",
  },
  {
    id: "control-engineering-depth",
    packPath: "packs/control-systems/v1.json",
  },
  { id: "signals-systems", packPath: "packs/signals-systems/v1.json" },
  { id: "electromagnetism", packPath: "packs/electromagnetism/v1.json" },
  {
    id: "thermodynamics-heat-transfer",
    packPath: "packs/thermodynamics-heat-transfer/v1.json",
  },
  { id: "fluid-mechanics", packPath: "packs/fluid-mechanics/v1.json" },
  {
    id: "calculus-analysis-clusters",
    lawIds: ["gradient-relation"],
    packPath: "packs/calculus-analysis/v1.json",
  },
  {
    id: "discrete-math-clusters",
    lawIds: ["handshaking-degree-sum", "two-set-inclusion-exclusion"],
    packPath: "packs/discrete-math/v1.json",
  },
  {
    id: "optimization-ml-clusters",
    packPath: "packs/optimization-ml/v1.json",
  },
  {
    id: "probability-statistics-depth",
    packPath: "packs/probability/v1.json",
  },
];

const packDirectories = (await readdir(new URL("packs/", root), { withFileTypes: true }))
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();
const packSources = new Map(await Promise.all(
  packDirectories.map(async (directory) => {
    const path = `packs/${directory}/v1.json`;
    return [
    path,
    await readFile(new URL(path, root), "utf8"),
    ] as const;
  }),
));
const wasm = await import("../lib/wasm/semath_wasm.js");
const wasmBytes = await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url));
await wasm.default({ module_or_path: wasmBytes });
const authoringReport = JSON.parse(new TextDecoder().decode(wasm.inspectPackCatalog(
  new TextEncoder().encode(JSON.stringify({
    schemaVersion: 3,
    sources: [...packSources].map(([path, source]) => ({ path, source })),
  })),
))) as PackAuthoringReport;
if (authoringReport.diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
  throw new Error("pack compiler rejected the engineering catalog");
}

let total = 0;
for (const suite of suites) {
  const pack = projectValidatedPack(
    JSON.parse(packSources.get(suite.packPath)!),
    authoringReport.forms,
  );
  const seed = scaffoldPackWorkspace(pack).corpus.cases.filter((item) =>
    !suite.lawIds || ("lawId" in item && suite.lawIds.includes(item.lawId))
  );
  const expected = (suite.lawIds?.length ?? pack.laws.length) * 10;
  if (seed.length !== expected) {
    throw new Error(`${suite.id}: expected ${expected} seed cases, got ${seed.length}`);
  }
  const corpus = generateLawDiversityCorpus(suite.id, seed, spec);
  const output = `${JSON.stringify(corpus, null, 2)}\n`;
  const path = new URL(`fixtures/corpus/${suite.id}.json`, root);
  if (check) {
    if (await readFile(path, "utf8").catch(() => "") !== output) {
      throw new Error(`${path.pathname}: generated corpus is stale`);
    }
  } else {
    await writeFile(path, output);
  }
  total += corpus.cases.length;
}

const crossFieldCorpus = engineeringCrossFieldCorpus();
const crossFieldOutput = `${JSON.stringify(crossFieldCorpus, null, 2)}\n`;
const crossFieldPath = new URL("fixtures/corpus/engineering-cross-field.json", root);
if (check) {
  if (await readFile(crossFieldPath, "utf8").catch(() => "") !== crossFieldOutput) {
    throw new Error(`${crossFieldPath.pathname}: generated corpus is stale`);
  }
} else {
  await writeFile(crossFieldPath, crossFieldOutput);
}
total += crossFieldCorpus.cases.length;

console.log(
  `${check ? "verified" : "generated"} ${suites.length + 1} engineering corpus files (${total} cases)`,
);

function engineeringCrossFieldCorpus(): Corpus {
  const examples = [
    ["a = b c", [["a", "pressure scalar"], ["b", "mass scalar"], ["c", "velocity scalar"]]],
    ["d = e f", [["d", "cyclic frequency scalar"], ["e", "electric charge scalar"], ["f", "electric field vector"]]],
    ["g = h i", [["g", "electric potential energy scalar"], ["h", "volumetric flow rate scalar"], ["i", "electric potential scalar"]]],
    ["j = k l", [["j", "pressure scalar"], ["k", "terminal voltage scalar"], ["l", "electric current scalar"]]],
    ["m = n o p", [["m", "electric charge scalar"], ["n", "mass scalar"], ["o", "specific heat capacity scalar"], ["p", "temperature change scalar"]]],
    ["q = r s t", [["q", "terminal voltage scalar"], ["r", "fluid density scalar"], ["s", "cross-sectional area scalar"], ["t", "velocity scalar"]]],
    ["u = v w x", [["u", "hydrostatic pressure scalar"], ["v", "fluid density scalar"], ["w", "gravitational acceleration scalar"], ["x", "time variable"]]],
    ["y = z / a", [["y", "heat-transfer rate scalar"], ["z", "thermal conductivity scalar"], ["a", "wall thickness scalar"]]],
    ["b = c - d", [["b", "terminal voltage scalar"], ["c", "heat added to the system scalar"], ["d", "work done by the system scalar"]]],
    ["e = 2 f g", [["e", "angular frequency scalar"], ["f", "fluid density scalar"], ["g", "cyclic frequency scalar"]]],
    ["h = 1 / i", [["h", "electric current scalar"], ["i", "signal period scalar"]]],
    ["j = k l", [["j", "wave propagation speed scalar"], ["k", "cyclic frequency scalar"], ["l", "time variable"]]],
    ["m = n \\frac{d o}{d p}", [["m", "terminal voltage scalar"], ["n", "capacitance scalar"], ["o", "electric current scalar"], ["p", "time variable"]]],
    ["q = R s + S t", [["q", "force vector"], ["R", "output matrix"], ["s", "state vector"], ["S", "feedthrough matrix"], ["t", "control input vector"]]],
    ["u = \\frac{1}{2} v w^2", [["u", "capacitance scalar"], ["v", "mass scalar"], ["w", "velocity scalar"]]],
    ["x = y z", [["x", "internal-energy change scalar"], ["y", "inductance scalar"], ["z", "electric current scalar"]]],
  ] as const;
  const topologies = ["single-document", "appendix-file", "context-file", "definitions-file", "nested-section"] as const;
  const proseFamilies = ["let-series", "suppose-that", "we-write", "respectively"] as const;
  const syntaxStructures = ["inline-math", "display-math", "equation-environment", "grouped-expression"] as const;
  const mutationFamilies = ["wrong-output", "wrong-input", "cross-domain-role", "dimension-collision", "shape-collision", "wrong-operator", "missing-term", "wrong-constant"] as const;
  return {
    cases: examples.map(([formula, roles], index) => {
      const topology = topologies[index % topologies.length]!;
      const proseFamily = proseFamilies[index % proseFamilies.length]!;
      const prose = crossFieldDeclaration(roles, proseFamily);
      const wrapped = wrapFormula(formula, syntaxStructures[index % syntaxStructures.length]!);
      const splitDeclarations = topology === "appendix-file" || topology === "definitions-file";
      const main = splitDeclarations
        ? `\\input{roles}\n${wrapped}`
        : `${prose}\n\n${wrapped}`;
      const documents = [{ content: main, fileId: "main", path: "main.tex" }];
      if (splitDeclarations) {
        documents.push({
          content: prose,
          fileId: "roles",
          path: topology === "appendix-file" ? "appendices/roles.tex" : "definitions.tex",
        });
      } else if (topology !== "single-document") {
        documents.push({
          content: "This supporting file intentionally contributes no declarations.",
          fileId: `context-${index + 1}`,
          path: topology === "nested-section" ? `sections/context-${index + 1}.md` : `context-${index + 1}.md`,
        });
      }
      return {
        cursor: { edge: index % 2 ? "after" as const : "before" as const, fileId: "main", needle: formula },
        diversity: {
          batch: "engineering-cross-field",
          mutationFamily: mutationFamilies[index % mutationFamilies.length]!,
          projectTopology: topology,
          proseFamily,
          semanticSkeleton: `cross-field-${index + 1}`,
          syntaxStructure: syntaxStructures[index % syntaxStructures.length]!,
        },
        documents,
        expectation: "refused" as const,
        id: `engineering-cross-field-${String(index + 1).padStart(2, "0")}`,
        refusalCategory: "cross-field-role-collision",
        variationTags: ["hard-negative", "safe-refusal", "role-conflict", "shape-explicit", "conventional-notation"],
      };
    }),
    domain: "engineering-cross-field",
    schemaVersion: 2,
  };
}

function crossFieldDeclaration(
  roles: readonly (readonly [string, string])[],
  family: "let-series" | "respectively" | "suppose-that" | "we-write",
): string {
  if (family === "respectively") {
    return `Let ${englishList(roles.map(([symbol]) => `$${symbol}$`))} denote ${englishList(roles.map(([, description]) => description))}, respectively.`;
  }
  if (family === "we-write") {
    return roles.map(([symbol, description]) => `We write $${symbol}$ for ${description}.`).join(" ");
  }
  const verb = family === "suppose-that" ? "Suppose" : "Let";
  const predicate = family === "suppose-that" ? "is" : "denote";
  return `${verb} ${englishList(roles.map(([symbol, description]) => `$${symbol}$ ${predicate} ${description}`))}.`;
}

function wrapFormula(formula: string, syntax: string): string {
  if (syntax === "display-math") return `\\[${formula}\\]`;
  if (syntax === "equation-environment") return `\\begin{equation}${formula}\\end{equation}`;
  if (syntax === "grouped-expression") return `$ {${formula}} $`;
  return `$${formula}$`;
}

function englishList(values: readonly string[]): string {
  if (values.length < 2) return values[0] ?? "";
  if (values.length === 2) return `${values[0]} and ${values[1]}`;
  return `${values.slice(0, -1).join(", ")}, and ${values.at(-1)}`;
}
