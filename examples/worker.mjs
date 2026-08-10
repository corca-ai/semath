import { SemathWorkerEngine } from "semath/worker";
import { SEMATH_PROTOCOL_VERSION } from "semath/protocol";
import { createProjectSnapshot } from "semath/wasmtex-adapter";
import { LatexSyntaxService } from "wasmtex/syntax";

const epoch = "standalone-worker-example";
const engine = await SemathWorkerEngine.create(() => import("semath/wasm"));
const content = "Let $x$ denote the input. Use $x$.";
const source = {
  fileId: "main",
  path: "main.md",
  language: "markdown",
  content,
  documentVersion: 1,
};
const syntax = new LatexSyntaxService().upsert(source);
engine.reset(createProjectSnapshot({
  documents: [{ content, language: "markdown", syntax }],
  epoch,
  inventoryVersion: 1,
  projectId: "example",
  mainFileId: "main",
}));
const result = engine.query({
  protocolVersion: SEMATH_PROTOCOL_VERSION,
  epoch,
  inventoryVersion: 1,
  documentVersion: 1,
  analysisGeneration: 1,
  query: { kind: "semanticView", fileId: "main", offset: content.lastIndexOf("x") },
});
engine.dispose();

if (result.value.kind !== "semanticView" || result.value.view.symbol?.symbol !== "x") {
  throw new Error("standalone Worker example did not resolve x");
}
console.log("standalone Worker example OK: resolved x");
