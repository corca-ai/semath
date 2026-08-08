import { readdir, readFile, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";

const fixturesRoot = new URL("../fixtures/", import.meta.url);
const versions = await readdir(fixturesRoot, { withFileTypes: true });
const build = spawnSync("cargo", ["build", "--locked", "-p", "semath-native"], {
  encoding: "utf8",
});
if (build.status !== 0) throw new Error(build.stderr || "native build failed");

for (const version of versions) {
  if (!version.isDirectory()) continue;
  const directory = new URL(`${version.name}/`, fixturesRoot);
  const files = await readdir(directory);
  for (const name of files.filter((file) => file.endsWith(".golden.json"))) {
    const fixtureName = name.replace(".golden.json", ".json");
    if (!files.includes(fixtureName)) continue;
    const fixture = await readFile(new URL(fixtureName, directory), "utf8");
    const native = spawnSync("./target/debug/semath-native", [], {
      encoding: "utf8",
      input: fixture,
    });
    if (native.status !== 0) throw new Error(native.stderr || `${fixtureName} failed`);
    const results = JSON.parse(native.stdout);
    const relativePath = `fixtures/${version.name}/${name}`;
    const committed = spawnSync("git", ["show", `HEAD:${relativePath}`], {
      encoding: "utf8",
    });
    const existingText =
      committed.status === 0
        ? committed.stdout
        : await readFile(new URL(name, directory), "utf8");
    const golden = JSON.parse(existingText);
    golden.results = results;
    const nextText = formatGolden(golden, existingText);
    await writeFile(
      new URL(name, directory),
      `${nextText}\n`,
      "utf8",
    );
    console.log(`updated ${version.name}/${name}`);
  }
}

function formatGolden(golden, previous) {
  if (!previous.includes("\n")) return JSON.stringify(golden);
  if (previous.includes('"results": [\n    {\n')) {
    return JSON.stringify(golden, null, 2);
  }
  const lines = ["{"];
  const entries = Object.entries(golden);
  for (const [index, [key, value]] of entries.entries()) {
    const comma = index === entries.length - 1 ? "" : ",";
    if (key !== "results") {
      lines.push(`  ${JSON.stringify(key)}: ${JSON.stringify(value)}${comma}`);
      continue;
    }
    lines.push('  "results": [');
    for (const [resultIndex, result] of value.entries()) {
      const resultComma = resultIndex === value.length - 1 ? "" : ",";
      lines.push(`    ${JSON.stringify(result)}${resultComma}`);
    }
    lines.push(`  ]${comma}`);
  }
  lines.push("}");
  return lines.join("\n");
}
