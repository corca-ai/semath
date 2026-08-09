import { spawnSync } from "node:child_process";
import corpus from "../fixtures/v0.14/scientific-foundation.json";
import {
  assertScientificResults,
  buildScientificFixture,
  type ScientificCorpus,
} from "./v0.14-scientific-fixture";

const scientific = buildScientificFixture(corpus as ScientificCorpus);
const native = spawnSync("cargo", ["run", "--quiet", "-p", "semath-native"], {
  encoding: "utf8",
  input: JSON.stringify(scientific.fixture),
});
if (native.status !== 0) {
  throw new Error(native.stderr || "v0.14 scientific fixture failed");
}
const results = JSON.parse(native.stdout);
const summary = assertScientificResults(results, scientific.expectations);
console.log(`v0.14 scientific foundation OK: ${summary.queries} exact vertical queries`);
