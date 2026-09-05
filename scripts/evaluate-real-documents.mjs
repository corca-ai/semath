import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { LatexSyntaxService } from "wasmtex/syntax";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index.ts";
import { SEMATH_PROTOCOL_VERSION } from "../packages/protocol/src/index.ts";
import { firstDifferentialFailure } from "./testing/differential.ts";

const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const sources = JSON.parse(await readFile("fixtures/real-documents/sources.json", "utf8"));
const tasks = JSON.parse(await readFile("fixtures/real-documents/tasks.json", "utf8"));
const encode = value => new TextEncoder().encode(JSON.stringify(value));
const decode = value => JSON.parse(new TextDecoder().decode(value));
const workerId = process.argv[2];

if (workerId) {
  const source = sources.find(source => source.id === workerId);
  assert.ok(source, "unknown corpus document");
  console.log(JSON.stringify(await evaluate(source)));
} else {
  const build = spawnSync("cargo", ["build", "--release", "--quiet", "--locked", "-p", "semath-native"], { encoding: "utf8" });
  assert.equal(build.status, 0, build.stderr);
  const report = {
    sourceCommit: spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim(),
    dirty: spawnSync("git", ["status", "--porcelain"], { encoding: "utf8" }).stdout.length > 0,
    platform: `${process.platform}/${process.arch}`,
    wasmSha256: sha256(await readFile("lib/wasm/semath_wasm_bg.wasm")),
    tasksSha256: sha256(await readFile("fixtures/real-documents/tasks.json")),
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    complete: false,
    expectedTasks: tasks.length,
    sources: [],
  };
  await mkdir(".artifacts/real-documents", { recursive: true });
  await writeFile(".artifacts/real-documents/report.json", JSON.stringify(report, null, 2) + "\n");
  for (const source of sources) {
    // Isolate both runtimes: a synchronous WASM call must not hang the corpus run.
    const worker = spawnSync(process.execPath, [fileURLToPath(import.meta.url), source.id], {
      encoding: "utf8", timeout: 120_000, maxBuffer: 32 * 1024 * 1024,
    });
    let entry;
    if (worker.error?.code === "ETIMEDOUT") {
      entry = failure(source, "timeout", "120 second per-document evaluation limit; native/WASM parity not verified");
    } else {
      assert.equal(worker.status, 0, `${source.id}: ${worker.stderr || worker.error}`);
      entry = JSON.parse(worker.stdout);
    }
    report.sources.push(entry);
    await writeFile(".artifacts/real-documents/report.json", JSON.stringify(report, null, 2) + "\n");
    console.log(`${source.id}: ${entry.observations.map(task => `${task.id}=${task.outcome}`).join(", ")}; ${entry.diagnostics?.reduce((count, item) => count + item.diagnostics.length, 0) ?? "unavailable"} diagnostics`);
  }
  report.complete = true;
  await writeFile(".artifacts/real-documents/report.json", JSON.stringify(report, null, 2) + "\n");
  console.log("Report: .artifacts/real-documents/report.json (development observations; not an accuracy gate)");
}

function failure(source, status, error) {
  return {
    id: source.id, field: source.field, archiveSha256: source.archiveSha256, status, error,
    observations: tasks.filter(task => task.document === source.id).map(task => ({ id: task.id, outcome: status })),
  };
}

async function evaluate(source) {
  const inputs = [];
  for (const file of source.files) {
    const bytes = await readFile(`.artifacts/real-documents/${source.id}/${file.path}`);
    assert.equal(sha256(bytes), file.sha256, `${source.id}/${file.path}: source drift`);
    // Preserve all TeX files and author macros in supplied style/class files.
    if (!/\.(tex|sty|cls)$/.test(file.path)) continue;
    const content = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    inputs.push({ content, fileId: file.path, path: file.path, documentVersion: 1, language: "latex" });
  }
  const syntax = new LatexSyntaxService();
  const started = performance.now();
  syntax.reset({ documents: inputs });
  const documents = inputs.map(input => adaptWasmtexDocument({ content: input.content, language: input.language, syntax: syntax.getFile(input.fileId) }));
  const snapshot = { documents, epoch: source.id, inventoryVersion: 1, mainFileId: source.main, projectId: source.id, protocolVersion: SEMATH_PROTOCOL_VERSION };
  const cases = tasks.filter(task => task.document === source.id);
  const queries = cases.flatMap(task => {
    const input = inputs.find(input => input.fileId === task.file);
    assert.equal(input.content.slice(task.offset, task.offset + task.symbol.length), task.symbol, `${task.id}: stale use annotation`);
    const declaration = inputs.find(input => input.fileId === task.definition.file);
    assert.equal(declaration.content.slice(task.definition.startOffset, task.definition.endOffset), task.symbol, `${task.id}: stale definition annotation`);
    return [
      { kind: "definition", fileId: task.file, offset: task.offset },
      { kind: "rename", fileId: task.file, offset: task.offset, newName: "w" },
    ];
  });
  queries.push(...inputs.filter(input => input.path.endsWith(".tex")).map(input => ({ kind: "diagnostics", fileId: input.fileId })));
  const envelopes = queries.map(query => ({ query, epoch: source.id, inventoryVersion: 1, documentVersion: 1, analysisGeneration: 0, protocolVersion: SEMATH_PROTOCOL_VERSION }));
  const native = spawnSync("target/release/semath-native", [], { input: JSON.stringify({ snapshot, queries: envelopes }), encoding: "utf8", maxBuffer: 32 * 1024 * 1024, timeout: 60_000 });
  if (native.error?.code === "ETIMEDOUT") return failure(source, "native-timeout", "Native exceeded 60 seconds; WASM not checked");
  const rejected = native.status !== 0;
  if (rejected) assert.match(native.stderr, /InvalidSyntaxSnapshot/, `${source.id}: unexpected native failure: ${native.error ?? native.stderr}`);
  const results = rejected ? undefined : JSON.parse(native.stdout);
  await init({ module_or_path: await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)) });
  const engine = new SemathEngine();
  let wasm;
  let wasmError;
  try {
    const { documents, ...metadata } = snapshot;
    engine.beginReset(encode(metadata));
    for (const document of documents) engine.ingestResetDocument(encode(document));
    engine.finishReset();
    wasm = envelopes.map(query => decode(engine.query(encode(query))));
  } catch (error) { wasmError = String(error); } finally { engine.free(); }
  if (rejected) {
    const reason = native.stderr.match(/InvalidSyntaxSnapshot\("(.*)"\)/)?.[1];
    assert.ok(reason && wasmError?.includes(reason), `${source.id}: inconsistent WASM rejection: ${wasmError}`);
    return failure(source, "input-rejected", reason);
  }
  assert.equal(wasmError, undefined, `${source.id}: WASM failed`);
  assert.equal(firstDifferentialFailure([{ name: "native", value: results }, { name: "wasm", value: wasm }]), undefined, `${source.id}: native/WASM mismatch`);
  const observations = cases.map((task, index) => {
    const definition = results[index * 2].value;
    const rename = results[index * 2 + 1].value;
    const exact = definition.locations.length === 1 && definition.locations.every(location => location.fileId === task.definition.file && location.range.startOffset === task.definition.startOffset && location.range.endOffset === task.definition.endOffset);
    return { id: task.id, outcome: exact ? "correct" : definition.locations.length === 0 ? "abstained" : "wrong-target", definition, rename };
  });
  const diagnostics = results.slice(cases.length * 2).map((result, index) => ({ file: queries[cases.length * 2 + index].fileId, ...result.value }));
  return {
    id: source.id, field: source.field, archiveSha256: source.archiveSha256, status: "analyzed",
    texFiles: inputs.filter(input => input.path.endsWith(".tex")).length,
    sourceBytes: inputs.reduce((total, input) => total + Buffer.byteLength(input.content), 0),
    mathRoots: documents.reduce((total, document) => total + document.mathRoots.length, 0),
    elapsedMs: Math.round(performance.now() - started), observations, diagnostics,
  };
}
