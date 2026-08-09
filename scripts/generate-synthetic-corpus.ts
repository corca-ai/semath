import { readFile, writeFile } from "node:fs/promises";
import {
  annotateCorpus,
  type Corpus,
  generateGlobalRefusalCorpus,
  generateLawDiversityCorpus,
  parseSyntheticDiversitySpec,
} from "../packages/evaluation/src/index";

const root = new URL("../", import.meta.url);
const check = process.argv.includes("--check");
const spec = parseSyntheticDiversitySpec(
  JSON.parse(await readFile(new URL("fixtures/synthetic-diversity-spec.json", root), "utf8")),
);

const groups = [
  {
    base: "fixtures/corpus/circuits.json",
    generated: "fixtures/corpus/circuits-diversity.json",
    generatedDomain: "circuits-diversity",
  },
  {
    base: "fixtures/corpus/control-systems.json",
    generated: "fixtures/corpus/control-systems-diversity.json",
    generatedDomain: "control-systems-diversity",
  },
  {
    base: "fixtures/corpus/mechanics.json",
    generated: "fixtures/corpus/mechanics-diversity.json",
    generatedDomain: "mechanics-diversity",
  },
  {
    base: "fixtures/corpus/linear-algebra-probe.json",
    generated: "fixtures/corpus/linear-algebra-diversity.json",
    generatedDomain: "linear-algebra-diversity",
  },
  {
    base: "fixtures/corpus/probability-probe.json",
    generated: "fixtures/corpus/probability-diversity.json",
    generatedDomain: "probability-diversity",
  },
] as const;

const outputs = new Map<string, Corpus>();
for (const group of groups) {
  const raw = JSON.parse(await readFile(new URL(group.base, root), "utf8")) as Corpus;
  const baseline = annotateCorpus(raw);
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
