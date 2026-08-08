import { SemathLspServer } from "semath/lsp";
import { SemathWorkerEngine } from "semath/worker";

const responses = [];
const semath = await SemathWorkerEngine.create(() => import("semath/wasm"));
const server = new SemathLspServer((message) => responses.push(message), {
  semath,
});
await server.handle({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
await server.handle({
  jsonrpc: "2.0",
  method: "textDocument/didOpen",
  params: {
    textDocument: {
      uri: "file:///example/main.md",
      languageId: "markdown",
      version: 1,
      text: "Let $x$ denote the input. Use $x$.",
    },
  },
});
await server.handle({
  jsonrpc: "2.0",
  id: 2,
  method: "textDocument/hover",
  params: {
    textDocument: { uri: "file:///example/main.md" },
    position: { line: 0, character: 31 },
  },
});
server.dispose();

const initialize = responses.find((message) => message.id === 1);
const hover = responses.find((message) => message.id === 2);
if (!initialize?.result?.capabilities || !hover?.result?.contents) {
  throw new Error("standalone LSP example did not initialize and return hover");
}
console.log("standalone LSP example OK: initialize + hover");
