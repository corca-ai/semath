import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const temporary = await mkdtemp(join(tmpdir(), "semath-consumer-"));
try {
  const packed = run("npm", ["pack", "--json", "--pack-destination", temporary]);
  const metadata = JSON.parse(packed)[0];
  const required = [
    "lib/wasm/semath_wasm_bg.wasm",
    "lib/wasm/semath_wasm.d.ts",
    "examples/worker.mjs",
    "examples/lsp.mjs",
  ];
  const names = new Set(metadata.files.map((file) => file.path));
  for (const path of required) {
    if (!names.has(path)) throw new Error(`packed release is missing ${path}`);
  }
  const tarball = join(temporary, metadata.filename);
  await writeFile(
    join(temporary, "package.json"),
    JSON.stringify({ name: "semath-clean-consumer", private: true, type: "module" }),
  );
  run("bun", ["add", tarball], temporary);
  run("bun", ["node_modules/semath/examples/worker.mjs"], temporary);
  run("bun", ["node_modules/semath/examples/lsp.mjs"], temporary);
  const sums = await readFile(
    join(temporary, "node_modules/semath/lib/wasm/SHA256SUMS"),
    "utf8",
  );
  const checksum = run(
    "shasum",
    ["-a", "256", "node_modules/semath/lib/wasm/semath_wasm_bg.wasm"],
    temporary,
  ).split(/\s+/)[0];
  if (!sums.includes(checksum)) {
    throw new Error("installed WASM does not match the published checksum");
  }
  console.log(`package smoke OK: ${metadata.filename} (${metadata.size} bytes)`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}

function run(command, args, cwd = process.cwd()) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${command} failed`);
  }
  return result.stdout.trim();
}
