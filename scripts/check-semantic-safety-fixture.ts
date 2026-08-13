import { readdir, readFile } from "node:fs/promises";
import {
  planSemanticSafetySuite,
  parseSemanticSafetySpec,
  SEMANTIC_SAFETY_CONTRACTS,
  type SemanticSafetyLawCatalogEntry,
} from "../packages/evaluation/src/semantic-safety";
import { validateFixtureSource } from "../packages/evaluation/src/synthetic";

const root = new URL("../", import.meta.url);
const fixtureUrl = new URL(
  "fixtures/development/semantic-safety-v1.json",
  root,
);

export async function loadSemanticSafetyLawCatalog(): Promise<
  SemanticSafetyLawCatalogEntry[]
> {
  const packsUrl = new URL("packs/", root);
  const directories = (await readdir(packsUrl, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .sort((left, right) => left.name.localeCompare(right.name));
  const entries: SemanticSafetyLawCatalogEntry[] = [];
  for (const directory of directories) {
    const pack = JSON.parse(
      await readFile(new URL(`${directory.name}/v1.json`, packsUrl), "utf8"),
    ) as {
      laws?: readonly {
        id: string;
        roles: readonly { id: string }[];
      }[];
    };
    for (const law of pack.laws ?? []) {
      entries.push({
        lawId: law.id,
        roles: law.roles.map((role) => role.id),
      });
    }
  }
  return entries;
}

export async function loadSemanticSafetySpec() {
  const [fixture, catalog] = await Promise.all([
    readFile(fixtureUrl, "utf8").then((source) => JSON.parse(source)),
    loadSemanticSafetyLawCatalog(),
  ]);
  return parseSemanticSafetySpec(fixture, catalog);
}

if (import.meta.main) {
  const spec = await loadSemanticSafetySpec();
  const plan = planSemanticSafetySuite(spec);
  const sourceFailures = spec.cases.flatMap((item) =>
    item.snapshots.flatMap((snapshot) =>
      snapshot.documents.flatMap((document) =>
        validateFixtureSource(document.content).map(
          (failure) => `${item.id}/${snapshot.id}/${document.fileId}: ${failure}`,
        ),
      ),
    ),
  );
  if (sourceFailures.length) {
    throw new Error(`semantic safety fixture is invalid:\n${sourceFailures.join("\n")}`);
  }
  const second = planSemanticSafetySuite(spec);
  if (JSON.stringify(plan) !== JSON.stringify(second)) {
    throw new Error("semantic safety plan is not deterministic");
  }
  const transforms = plan.filter((item) => item.transform !== "identity").length;
  console.log(
    `semantic safety fixture OK: ${spec.cases.length} seeds, ${plan.length} planned probes, ${transforms} metamorphic variants, ${SEMANTIC_SAFETY_CONTRACTS.length} contracts`,
  );
}
