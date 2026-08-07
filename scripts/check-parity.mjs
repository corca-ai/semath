import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";

const fixtureSets = [
  {
    fixtureUrl: new URL("../fixtures/v0.1/explicit-definition.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.1/explicit-definition.golden.json", import.meta.url),
    version: "v0.1",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.2/bound-variable-rename.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.2/bound-variable-rename.golden.json", import.meta.url),
    version: "v0.2",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.3/shape-diagnostics.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.3/shape-diagnostics.golden.json", import.meta.url),
    version: "v0.3",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.4/formula-intelligence.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.4/formula-intelligence.golden.json", import.meta.url),
    version: "v0.4",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/symbol-inspection.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/symbol-inspection.golden.json", import.meta.url),
    version: "v0.5",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/prose-shape-claims.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/prose-shape-claims.golden.json", import.meta.url),
    version: "v0.5-prose",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/scoped-domain-evidence.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/scoped-domain-evidence.golden.json", import.meta.url),
    version: "v0.5-domains",
  },
  {
    fixtureUrl: new URL("../fixtures/v0.5/notation-consistency.json", import.meta.url),
    goldenUrl: new URL("../fixtures/v0.5/notation-consistency.golden.json", import.meta.url),
    version: "v0.5-consistency",
  },
];

const build = spawnSync("cargo", ["build", "--locked", "-p", "semath-native"], {
  encoding: "utf8",
});
if (build.status !== 0) throw new Error(build.stderr || "native build failed");
await init({
  module_or_path: await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)),
});
const encoder = new TextEncoder();
const decoder = new TextDecoder();

for (const { fixtureUrl, goldenUrl, version } of fixtureSets) {
  const fixtureText = await readFile(fixtureUrl, "utf8");
  const fixture = JSON.parse(fixtureText);
  const native = spawnSync("./target/debug/semath-native", [], {
    encoding: "utf8",
    input: fixtureText,
  });
  if (native.status !== 0) throw new Error(native.stderr || "native fixture failed");
  const nativeResults = JSON.parse(native.stdout);

  const engine = new SemathEngine();
  engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
  const wasmResults = fixture.queries.map((query) =>
    JSON.parse(decoder.decode(engine.query(encoder.encode(JSON.stringify(query))))),
  );
  engine.free();

  const nativeJson = JSON.stringify(nativeResults);
  const wasmJson = JSON.stringify(wasmResults);
  if (nativeJson !== wasmJson) {
    throw new Error(`native/WASM semantic result mismatch for ${version}`);
  }
  const golden = JSON.parse(await readFile(goldenUrl, "utf8"));
  if (nativeJson !== JSON.stringify(golden.results)) {
    throw new Error(`semantic result differs from the ${version} golden fixture`);
  }
  console.log(`parity OK: ${fixture.queries.length} ${version} queries`);
}
