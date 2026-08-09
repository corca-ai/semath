import { SemathWorkerEngine } from "semath/worker";

const epoch = "standalone-worker-example";
const engine = await SemathWorkerEngine.create(() => import("semath/wasm"));
const content = "Let $x$ denote the input. Use $x$.";
const mathRegions = [...content.matchAll(/\$([^$]+)\$/g)].map((match) => ({
  closed: true,
  contentRange: {
    startOffset: match.index + 1,
    endOffset: match.index + match[0].length - 1,
  },
  delimiter: "$",
  fullRange: {
    startOffset: match.index,
    endOffset: match.index + match[0].length,
  },
}));
engine.reset({
  protocolVersion: 3,
  epoch,
  inventoryVersion: 1,
  projectId: "example",
  mainFileId: "main",
  documents: [
    {
      fileId: "main",
      path: "main.md",
      language: "markdown",
      content,
      documentVersion: 1,
      includes: [],
      macros: [],
      mathRegions,
    },
  ],
});
const result = engine.query({
  protocolVersion: 3,
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
