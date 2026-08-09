import { checkPackConformance } from "../packages/evaluation/src/index";
import {
  loadFoundationFixtures,
  loadPackCatalog,
  loadQualityFixtures,
} from "./evaluation-fixtures";

const [{ corpora, manifest }, catalog] = await Promise.all([
  loadQualityFixtures(),
  loadPackCatalog(),
]);
const foundations = await loadFoundationFixtures(manifest);
const report = checkPackConformance(
  manifest,
  catalog,
  corpora,
  new Map([...foundations].map(([id, corpus]) => [id, corpus.cases.length])),
);
for (const pack of report.packs) {
  console.log(
    `${pack.packId}: summary=${pack.summary} laws=${pack.coveredLaws}/${pack.laws} authoredCases=${pack.authoredCases} capabilities=${Object.entries(pack.capabilities).map(([id, maturity]) => `${id}:${maturity}`).join(",")}`,
  );
}
if (report.failures.length) {
  throw new Error(`pack conformance failed:\n${report.failures.join("\n")}`);
}
console.log(`pack conformance OK: ${report.packs.length} built-in packs`);
