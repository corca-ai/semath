import { readFile, writeFile } from "node:fs/promises";
import {
  annotateCorpus,
  buildPromotionSeedCorpus,
  type Corpus,
  generateGlobalRefusalCorpus,
  generateLawDiversityCorpus,
  parseSyntheticDiversitySpec,
  parsePromotionSeedSpec,
} from "../packages/evaluation/src/index";

const root = new URL("../", import.meta.url);
const check = process.argv.includes("--check");
const evidenceStart = "% semath-recognition-evidence:start";
const evidenceEnd = "% semath-recognition-evidence:end";
const texCommandSymbols = new Set([
  "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta",
  "theta", "vartheta", "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron",
  "pi", "varpi", "rho", "varrho", "sigma", "varsigma", "tau", "upsilon", "phi",
  "varphi", "chi", "psi", "omega", "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi",
  "Sigma", "Upsilon", "Phi", "Psi", "Omega", "nabla",
]);
const spec = parseSyntheticDiversitySpec(
  JSON.parse(await readFile(new URL("fixtures/synthetic-diversity-spec.json", root), "utf8")),
);
const promotionSeeds = parsePromotionSeedSpec(
  JSON.parse(await readFile(new URL("fixtures/promotion-law-seeds.json", root), "utf8")),
);

const groups = [
  {
    base: "fixtures/corpus/circuits.json",
    generated: "fixtures/corpus/circuits-diversity.json",
    generatedDomain: "circuits-diversity",
    pack: "packs/circuits/v1.json",
  },
  {
    base: "fixtures/corpus/control-systems.json",
    generated: "fixtures/corpus/control-systems-diversity.json",
    generatedDomain: "control-systems-diversity",
    pack: "packs/control-systems/v1.json",
  },
  {
    base: "fixtures/corpus/mechanics.json",
    generated: "fixtures/corpus/mechanics-diversity.json",
    generatedDomain: "mechanics-diversity",
    pack: "packs/classical-mechanics/v1.json",
  },
  {
    base: "fixtures/corpus/linear-algebra-probe.json",
    generated: "fixtures/corpus/linear-algebra-diversity.json",
    generatedDomain: "linear-algebra-diversity",
    pack: "packs/linear-algebra/v1.json",
  },
  {
    base: "fixtures/corpus/probability-probe.json",
    generated: "fixtures/corpus/probability-diversity.json",
    generatedDomain: "probability-diversity",
    pack: "packs/probability/v1.json",
  },
] as const;

const outputs = new Map<string, Corpus>();
for (const group of groups) {
  const raw = JSON.parse(await readFile(new URL(group.base, root), "utf8")) as Corpus;
  const pack = JSON.parse(await readFile(new URL(group.pack, root), "utf8")) as PackSource;
  const baseline = annotateCorpus(withRecognitionEvidence(raw, pack));
  outputs.set(group.base, baseline);
  outputs.set(
    group.generated,
    generateLawDiversityCorpus(group.generatedDomain, baseline.cases, spec),
  );
}
outputs.set(
  "fixtures/corpus/global-adversarial.json",
  generateGlobalRefusalCorpus("global-adversarial", spec),
);
for (const suite of promotionSeeds.suites) {
  const pack = JSON.parse(
    await readFile(new URL(`packs/${suite.packId}/v1.json`, root), "utf8"),
  ) as PackSource;
  const baseline = annotateCorpus(withRecognitionEvidence(buildPromotionSeedCorpus(suite), pack));
  const diversityDomain = suite.id.replace(/-probe$/u, "-diversity");
  outputs.set(`fixtures/corpus/${suite.id}.json`, baseline);
  outputs.set(
    `fixtures/corpus/${diversityDomain}.json`,
    generateLawDiversityCorpus(diversityDomain, baseline.cases, spec),
  );
}

for (const [path, corpus] of outputs) {
  const next = `${JSON.stringify(corpus, null, 2)}\n`;
  const url = new URL(path, root);
  if (check) {
    const current = await readFile(url, "utf8").catch(() => "");
    if (current !== next) throw new Error(`${path}: generated corpus is stale`);
  } else {
    await writeFile(url, next);
  }
}

console.log(
  `${check ? "verified" : "generated"} ${outputs.size} corpus files (${[...outputs.values()].reduce((sum, corpus) => sum + corpus.cases.length, 0)} cases)`,
);

interface PackSource {
  laws: readonly {
    activationPhrases?: readonly string[];
    id: string;
    roles: readonly {
      concept: string;
      id: string;
      shape?: string;
      variadic?: boolean;
    }[];
  }[];
}

function withRecognitionEvidence(corpus: Corpus, pack: PackSource): Corpus {
  const laws = new Map(pack.laws.map((law) => [law.id, law]));
  return {
    ...corpus,
    cases: corpus.cases.map((item, index) => {
      if (item.expectation !== "recognized") return item;
      const law = laws.get(item.lawId);
      if (!law) throw new Error(`${corpus.domain}/${item.id}: unknown law ${item.lawId}`);
      const entries = Object.entries(item.expectedRoles).map(([roleId, symbol]) => {
        const role = law.roles.find((candidate) => candidate.id === roleId)
          ?? law.roles.find((candidate) =>
            candidate.id.endsWith(`-${roleId}`)
            || candidate.concept.endsWith(`:${roleId}`)
          )
          ?? law.roles.find((candidate) => candidate.variadic);
        if (!role) throw new Error(`${item.id}: unknown role ${roleId}`);
        const concept = role.concept.slice(role.concept.lastIndexOf(":") + 1).replaceAll("-", " ");
        const description = role.shape && !concept.includes(role.shape)
          ? `${concept} ${role.shape}`
          : concept;
        return { description, symbol: `$${texSymbol(symbol)}$` };
      });
      const evidence = declaration(entries, index);
      const activation = law.activationPhrases?.length
        ? law.activationPhrases[index % law.activationPhrases.length]
        : undefined;
      return {
        ...item,
        documents: item.documents.map((document) => {
          if (document.fileId !== item.cursor.fileId) return document;
          const content = document.content
            .replace(
              /% semath-recognition-evidence:start\n[\s\S]*?% semath-recognition-evidence:end\n/u,
              "",
            )
            .replace(/The reviewed law context states [^\n]+ for (?=\$|\\\[|\\\(|\\begin)/u, "");
          const needleOffset = content.indexOf(item.cursor.needle);
          let formulaOffset = [
            content.lastIndexOf("$", needleOffset),
            content.lastIndexOf("\\[", needleOffset),
            content.lastIndexOf("\\(", needleOffset),
            content.lastIndexOf("\\begin{equation}", needleOffset),
            content.lastIndexOf("\\begin{align}", needleOffset),
          ].reduce((latest, offset) => Math.max(latest, offset), -1);
          while (formulaOffset > 0 && content[formulaOffset - 1] === "$") formulaOffset -= 1;
          if (needleOffset < 0 || formulaOffset < 0) {
            throw new Error(`${item.id}: cannot place recognition evidence before the cursor formula`);
          }
          return {
            ...document,
            content: `${content.slice(0, formulaOffset)}${evidenceStart}\n${evidence}\n${evidenceEnd}\n${activation ? `The reviewed law context states ${activation} for ` : ""}${content.slice(formulaOffset)}`,
          };
        }),
      };
    }),
  };
}

function texSymbol(symbol: string): string {
  return texCommandSymbols.has(symbol) ? `\\${symbol}` : symbol;
}

function declaration(
  entries: readonly { description: string; symbol: string }[],
  index: number,
): string {
  const pairs = entries.map((entry) => `${entry.symbol} denotes ${entry.description}`);
  const letPairs = entries.map((entry) => `${entry.symbol} denote ${entry.description}`);
  const symbols = englishList(entries.map((entry) => entry.symbol));
  const descriptions = englishList(entries.map((entry) => entry.description));
  const variants = [
    `Let ${englishList(letPairs)}.`,
    `Let ${symbols} denote ${descriptions}, respectively.`,
    entries.map((entry) => `We write ${entry.symbol} for ${entry.description}.`).join(" "),
    entries.map((entry) => `${entry.symbol} denotes ${entry.description}.`).join(" "),
    `Here ${englishList(pairs)}.`,
  ];
  return variants[index % variants.length]!;
}

function englishList(values: readonly string[]): string {
  if (values.length < 2) return values[0] ?? "";
  if (values.length === 2) return `${values[0]} and ${values[1]}`;
  return `${values.slice(0, -1).join(", ")}, and ${values.at(-1)}`;
}
