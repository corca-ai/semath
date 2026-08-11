import { readdir, readFile } from "node:fs/promises";
import {
  type Corpus,
  type FoundationCorpus,
  type PackCatalogEntry,
  type QualityManifest,
  parseCorpus,
  parseFoundationCorpus,
  parseQualityManifest,
  summarizePack,
} from "../packages/evaluation/src/index";
import { materializeEngineeringCorpora } from "./generate-engineering-corpus";
import { materializeSyntheticCorpora } from "./generate-synthetic-corpus";

const fixturesRoot = new URL("../fixtures/", import.meta.url);
const packsRoot = new URL("../packs/", import.meta.url);
let generatedCorpora: Promise<Map<string, Corpus>> | undefined;

export async function loadQualityFixtures(): Promise<{
  corpora: Map<string, Corpus>;
  manifest: QualityManifest;
}> {
  const manifest = parseQualityManifest(
    JSON.parse(
      await readFile(new URL("corpus-manifest.json", fixturesRoot), "utf8"),
    ),
  );
  const generated = await loadGeneratedCorpora();
  const materialized = new Set(manifest.materializedSuiteIds);
  const corpora = new Map<string, Corpus>();
  for (const suite of manifest.suites) {
    const generatedSource = generated.get(suite.path);
    if (materialized.has(suite.id) !== Boolean(generatedSource)) {
      throw new Error(
        `${suite.id}: manifest materialization policy does not match ${suite.path}`,
      );
    }
    const source = generatedSource ?? JSON.parse(
      await readFile(new URL(suite.path, fixturesRoot), "utf8"),
    ) as unknown;
    corpora.set(suite.id, parseCorpus(source, suite));
  }
  for (const path of generated.keys()) {
    if (!manifest.suites.some((suite) => suite.path === path)) {
      throw new Error(`generated corpus has no manifest suite: ${path}`);
    }
  }
  return { corpora, manifest };
}

function loadGeneratedCorpora(): Promise<Map<string, Corpus>> {
  generatedCorpora ??= Promise.all([
    materializeSyntheticCorpora(),
    materializeEngineeringCorpora(),
  ]).then((groups) => {
    const corpora = new Map<string, Corpus>();
    for (const group of groups) {
      for (const [path, corpus] of group) {
        if (corpora.has(path)) throw new Error(`duplicate generated corpus path: ${path}`);
        corpora.set(path, corpus);
      }
    }
    return corpora;
  });
  return generatedCorpora;
}

export async function loadFoundationFixtures(
  manifest: QualityManifest,
): Promise<Map<string, FoundationCorpus>> {
  const corpora = new Map<string, FoundationCorpus>();
  for (const suite of manifest.foundationSuites) {
    const source = JSON.parse(
      await readFile(new URL(suite.path, fixturesRoot), "utf8"),
    ) as unknown;
    corpora.set(suite.id, parseFoundationCorpus(source, suite));
  }
  return corpora;
}

export async function loadPackCatalog(): Promise<PackCatalogEntry[]> {
  const directories = (await readdir(packsRoot, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  return Promise.all(
    directories.map(async (directory) => {
      const path = `${directory}/v1.json`;
      const source = JSON.parse(
        await readFile(new URL(path, packsRoot), "utf8"),
      ) as unknown;
      return summarizePack(source, path);
    }),
  );
}
