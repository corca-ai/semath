#!/usr/bin/env bun

import { readFile } from "node:fs/promises";
import { builtInPacks, loadPack } from "./src/index.ts";

const paths = process.argv.slice(2);
const inputs = paths.length
  ? await Promise.all(paths.map(async (path) => [path, await readFile(path, "utf8")]))
  : builtInPacks().map((pack) => [`built-in:${pack.packId}`, pack]);

let failed = false;
for (const [name, input] of inputs) {
  const result = loadPack(input);
  if (result.ok) {
    process.stdout.write(
      `${name}: ok (${result.pack.patterns.length} patterns, ${result.pack.rewrites.length} rewrites)\n`,
    );
    continue;
  }
  failed = true;
  for (const error of result.errors) {
    process.stderr.write(`${name}: ${error.path}: ${error.message}\n`);
  }
}

if (failed) process.exitCode = 1;
