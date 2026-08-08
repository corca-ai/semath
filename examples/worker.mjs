import { SemathWorkerEngine } from "semath/worker";

const epoch = "standalone-worker-example";
const engine = await SemathWorkerEngine.create(() => import("semath/wasm"));
engine.reset({
  protocolVersion: 1,
  epoch,
  inventoryVersion: 1,
  projectId: "example",
  mainFileId: "main",
  documents: [
    {
      fileId: "main",
      path: "main.md",
      language: "markdown",
      content: "Let $x$ denote the input. Use $x$.",
      documentVersion: 1,
    },
  ],
});
const result = engine.query({
  protocolVersion: 1,
  epoch,
  inventoryVersion: 1,
  documentVersion: 1,
  analysisGeneration: 1,
  query: { kind: "symbolInfo", fileId: "main", offset: 31 },
});
engine.dispose();

if (result.value.kind !== "symbolInfo" || result.value.info?.symbol !== "x") {
  throw new Error("standalone Worker example did not resolve x");
}
console.log("standalone Worker example OK: resolved x");
