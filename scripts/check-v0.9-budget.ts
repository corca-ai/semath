import { performance } from "node:perf_hooks";
import {
  createSemathLspServer,
  type JsonRpcMessage,
} from "../packages/lsp/src/index";

const COLD_START_BUDGET_MS = 1_500;
const WARM_P95_BUDGET_MS = 50;
const RESPONSE_BUDGET_BYTES = 256 * 1024;
const WARM_RUNS = 40;

const messages: JsonRpcMessage[] = [];
const coldStarted = performance.now();
const server = await createSemathLspServer((message) => messages.push(message), {
  epoch: "v0.9-budget",
  projectId: "v0.9-budget",
});
const coldStartMs = performance.now() - coldStarted;
const uri = "file:///main.tex";
const text = [
  "\\section{Model}\\label{sec:model}",
  "Let $p$ denote a probability distribution.",
  "See \\ref{sec:model} and use $p$.",
].join("\n");

await server.handle({
  method: "textDocument/didOpen",
  params: {
    textDocument: { languageId: "latex", text, uri, version: 1 },
  },
});
if (server.getRuntimeStats().syntax.parseCount !== 1) {
  throw new Error("opening one LaTeX revision must perform exactly one syntax parse");
}

const durations: number[] = [];
let maximumResponseBytes = 0;
for (let run = 0; run < WARM_RUNS; run += 1) {
  const id = run + 1;
  const before = messages.length;
  const started = performance.now();
  await server.handle({
    id,
    method: run % 2 === 0 ? "textDocument/hover" : "textDocument/definition",
    params: {
      position: run % 2 === 0 ? { character: 29, line: 2 } : { character: 11, line: 2 },
      textDocument: { uri },
    },
  });
  durations.push(performance.now() - started);
  const response = messages.slice(before).find((message) => message.id === id);
  maximumResponseBytes = Math.max(
    maximumResponseBytes,
    new TextEncoder().encode(JSON.stringify(response)).byteLength,
  );
}

if (server.getRuntimeStats().syntax.parseCount !== 1) {
  throw new Error("read-only authoring queries reparsed an unchanged document");
}

await server.handle({
  method: "textDocument/didChange",
  params: {
    contentChanges: [{ text: `${text}\nUpdated.` }],
    textDocument: { uri, version: 2 },
  },
});
if (server.getRuntimeStats().syntax.parseCount !== 2) {
  throw new Error("a new document revision must perform exactly one additional syntax parse");
}

durations.sort((left, right) => left - right);
const warmP95Ms = durations[Math.ceil(durations.length * 0.95) - 1]!;
server.dispose();

if (coldStartMs > COLD_START_BUDGET_MS) {
  throw new Error(`LSP cold start ${coldStartMs.toFixed(2)}ms exceeded ${COLD_START_BUDGET_MS}ms`);
}
if (warmP95Ms > WARM_P95_BUDGET_MS) {
  throw new Error(`LSP query p95 ${warmP95Ms.toFixed(2)}ms exceeded ${WARM_P95_BUDGET_MS}ms`);
}
if (maximumResponseBytes > RESPONSE_BUDGET_BYTES) {
  throw new Error(
    `LSP response ${maximumResponseBytes} bytes exceeded ${RESPONSE_BUDGET_BYTES} bytes`,
  );
}

console.log(
  `v0.9 budget OK: cold=${coldStartMs.toFixed(2)}ms warm-p95=${warmP95Ms.toFixed(2)}ms max-response=${maximumResponseBytes}B parses=2`,
);
