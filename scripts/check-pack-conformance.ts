import { checkPackConformance } from "../packages/evaluation/src/index";
import { loadPackCatalog, loadQualityFixtures } from "./evaluation-fixtures";

const [{ corpora, manifest }, catalog] = await Promise.all([
  loadQualityFixtures(),
  loadPackCatalog(),
]);
const report = checkPackConformance(manifest, catalog, corpora);
for (const pack of report.packs) {
  console.log(
    `${pack.packId}: tier=${pack.tier} laws=${pack.coveredLaws}/${pack.laws} authoredCases=${pack.authoredCases}`,
  );
}
if (report.failures.length) {
  throw new Error(`pack conformance failed:\n${report.failures.join("\n")}`);
}
console.log(`pack conformance OK: ${report.packs.length} built-in packs`);
