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

const fixturesRoot = new URL("../fixtures/", import.meta.url);
const packsRoot = new URL("../packs/", import.meta.url);

export async function loadQualityFixtures(): Promise<{
  corpora: Map<string, Corpus>;
  manifest: QualityManifest;
}> {
  const manifest = parseQualityManifest(
    JSON.parse(
      await readFile(new URL("corpus-manifest.json", fixturesRoot), "utf8"),
    ),
  );
  const corpora = new Map<string, Corpus>();
  for (const suite of manifest.suites) {
    const source = JSON.parse(
      await readFile(new URL(suite.path, fixturesRoot), "utf8"),
    ) as unknown;
    corpora.set(suite.id, parseCorpus(source, suite));
  }
  return { corpora, manifest };
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
