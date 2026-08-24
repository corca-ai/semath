import { readFile } from "node:fs/promises";
import { parseSpentHoldoutRegistry } from "../packages/evaluation/src/index";

const path = "fixtures/challenge/spent-holdout-registry-v1.json";
const registry = parseSpentHoldoutRegistry(
  JSON.parse(await readFile(path, "utf8")),
);
console.log(
  `spent holdout registry OK: ${registry.entries.length} terminal releases, ` +
    `${registry.entries.reduce((sum, entry) => sum + entry.lineage.scenarioIds.length, 0)} scenarios`,
);
