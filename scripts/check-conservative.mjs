import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { LatexSyntaxService } from "wasmtex/syntax";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index.ts";
import { SEMATH_PROTOCOL_VERSION } from "../packages/protocol/src/index.ts";
import { firstDifferentialFailure } from "./testing/differential.ts";

// Public acceptance examples use real syntax and independently stated expectations.
const cases = [
  {
    id: "equation-labels-are-not-variable-uses",
    symbol: "t",
    source: "\\[\\label{t}\\tag{t}x=0\\] Let $t$ denote duration. Inspect $t$.",
    needle: "$t$.",
    offset: 1,
    definition: true,
    diagnostics: [],
  },
  {
    id: "inactive-tex-source-cannot-authorize-navigation-or-edits",
    language: "latex",
    source: "\\begin{comment}Let $z$ denote displacement. Inspect $z$.\\end{comment}",
    needle: "$z$.",
    offset: 1,
    definition: false,
    diagnostics: [],
  },
  {
    id: "command-definition-hygiene-uses-source-spelling",
    symbol: "\\beta",
    rename: false,
    source: "Let $\\beta$ be a vector. Inspect $\\beta$.",
    needle: "$\\beta$.",
    offset: 1,
    definition: true,
    diagnostics: [],
  },
  {
    id: "display-environments-preserve-definition-visibility",
    source: "Let $z$ denote displacement. \\begin{align}z=0\\end{align}",
    needle: "z=0",
    definition: true,
    diagnostics: [],
  },
  {
    id: "definitions-survive-unrelated-equation-paragraphs",
    source: "Let $z$ denote displacement. $a=b$.\n\nAnother equation is $c=d$.\n\nInspect $z$.",
    needle: "$z$.",
    offset: 1,
    definition: true,
    diagnostics: [],
  },
  {
    id: "concessive-comparison-is-not-a-global-conflict",
    source: "The short kernel has $k<n$. Even with $k=n$, the cost stays bounded.",
    needle: "k=n",
    diagnostics: [],
    unproved: true,
  },
  ...[
    ["field-is-not-role-evidence", "For a periodic signal, the asserted relation is $f=1/T$.", "f=1/T"],
    ["law-name-is-not-role-evidence", "For the inductor, the passive sign convention is used. $v_L=L\\frac{di_L}{dt}$.", "v_L=L"],
  ].map(([id, source, needle]) => ({ id, source, needle, diagnostics: [], unproved: true, noTypedRelation: true })),
  {
    id: "definition-and-rename",
    source: "Let $z$ denote displacement. Inspect $z$.",
    needle: "$z$.",
    offset: 1,
    definition: true,
    diagnostics: [],
  },
  {
    id: "no-definition",
    source: "Inspect $z$.",
    needle: "$z$.",
    offset: 1,
    definition: false,
    diagnostics: [],
  },
  {
    id: "explicit-dimension-conflict",
    source: "Let $v$ be velocity, $m$ mass, and $t$ duration. $v=m/t$.",
    needle: "v=m/t",
    diagnostics: ["quantity-assignment-dimension-mismatch"],
  },
  {
    id: "compatible-dimensions",
    source: "Let $d$ be length and $s$ length. $d=s$.",
    needle: "d=s",
    diagnostics: [],
  },
  {
    id: "missing-dimensions",
    source: "The recorded equation is $d=t$.",
    needle: "d=t",
    diagnostics: [],
    unproved: true,
  },
  {
    id: "explicit-unit-conflict",
    source: "Let $m$ be mass in seconds. Inspect $m$.",
    needle: "$m$",
    offset: 1,
    diagnostics: ["quantity-unit-dimension-mismatch"],
  },
  {
    id: "compatible-unit",
    source: "Let $m$ be mass in kilograms. Inspect $m$.",
    needle: "$m$",
    offset: 1,
    diagnostics: [],
  },
  {
    id: "explicit-shape-conflict",
    source: "Let $A$ be a 2 by 3 matrix. Let $x$ be a 4-dimensional vector. $Ax$.",
    needle: "Ax",
    diagnostics: ["constraint-product-shape-conflict"],
  },
  {
    id: "compatible-shapes",
    source: "Let $A$ be a 2 by 3 matrix. Let $x$ be a 3-dimensional vector. $Ax$.",
    needle: "Ax",
    diagnostics: [],
  },
  ...[
    "This equation does not contradict the stated assumptions.",
    "If this equation were incorrect, we would revise the model.",
    "The reviewer asked whether this equation is incorrect.",
    "The diagram can be red or blue.",
    "The formula remains ambiguous.",
    "The reviewer says this equation is incorrect.",
  ].map((suffix, index) => ({
    id: `prose-is-not-proof-${index + 1}`,
    source: `The recorded equation is $q=Av$. ${suffix}`,
    needle: "q=Av",
    diagnostics: [],
    unproved: true,
  })),
  {
    id: "formula-label-is-not-proof",
    source: "The selected continuum identity is $q=Av$.",
    needle: "q=Av",
    diagnostics: [],
    unproved: true,
  },
];

const nativeOnly = process.argv.includes("--native-only");
const build = spawnSync("cargo", ["build", "--quiet", "--locked", "-p", "semath-native"], { encoding: "utf8" });
assert.equal(build.status, 0, build.stderr);
if (!nativeOnly) await init({ module_or_path: await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)) });

const encode = (value) => new TextEncoder().encode(JSON.stringify(value));
const decode = (value) => JSON.parse(new TextDecoder().decode(value));
let observations = 0;
for (const language of ["latex", "markdown"]) {
  for (const item of cases) {
    if (item.language && item.language !== language) continue;
    const source = { content: item.source, documentVersion: 1, fileId: "main", language, path: language === "latex" ? "main.tex" : "main.md" };
    const syntax = new LatexSyntaxService();
    syntax.reset({ documents: [source] });
    const document = adaptWasmtexDocument({ content: source.content, language, syntax: syntax.getFile("main") });
    const snapshot = { documents: [document], epoch: "conservative", inventoryVersion: 1, mainFileId: "main", projectId: "conservative", protocolVersion: SEMATH_PROTOCOL_VERSION };
    const offset = source.content.lastIndexOf(item.needle) + (item.offset ?? 0);
    const queries = [
      { kind: "semanticView", fileId: "main", offset },
      { kind: "diagnostics", fileId: "main" },
      { kind: "definition", fileId: "main", offset },
      { kind: "references", fileId: "main", offset, includeDeclaration: true },
      { kind: "rename", fileId: "main", offset, newName: "w" },
    ].map((query) => ({ query, epoch: snapshot.epoch, inventoryVersion: 1, documentVersion: 1, analysisGeneration: 0, protocolVersion: SEMATH_PROTOCOL_VERSION }));
    const results = nativeQueries(snapshot, queries);
    const label = `${item.id}/${language}`;
    verify(item, source, results, label);
    if (!nativeOnly) {
      const engine = new SemathEngine();
      try {
        const { documents, ...metadata } = snapshot;
        engine.beginReset(encode(metadata));
        for (const entry of documents) engine.ingestResetDocument(encode(entry));
        engine.finishReset();
        const wasm = queries.map((query) => decode(engine.query(encode(query))));
        assertParity(results, wasm, `${label}: native/WASM parity`);
        if (item.definition === true) {
          // Remove the actual source declaration, then compare the live engine
          // with a native reset at that same document and inventory revision.
          const changedSource = { ...source, content: `Inspect $${item.symbol ?? "z"}$.`, documentVersion: 2 };
          syntax.upsert(changedSource);
          const changedDocument = adaptWasmtexDocument({ content: changedSource.content, language, syntax: syntax.getFile("main") });
          const changedSnapshot = { ...snapshot, documents: [changedDocument], inventoryVersion: 2 };
          engine.applyChanges(encode({ changes: [{ kind: "upsert", document: changedDocument }], epoch: snapshot.epoch, inventoryVersion: 2, analysisGeneration: 1, protocolVersion: SEMATH_PROTOCOL_VERSION }));
          const changedQueries = queries.map((entry) => ({
            ...entry,
            documentVersion: 2,
            inventoryVersion: 2,
            analysisGeneration: 1,
            query: entry.query.kind === "diagnostics" ? entry.query : { ...entry.query, offset: changedSource.content.indexOf(item.symbol ?? "z") },
          }));
          const rebuilt = nativeQueries(changedSnapshot, changedQueries);
          const incremental = changedQueries.map((query) => decode(engine.query(encode(query))));
          verify({ ...item, definition: false }, changedSource, incremental, `${label}/declaration-removed`);
          assertParity(rebuilt, incremental, `${label}: native clean/WASM incremental retraction`);
        }
      } finally {
        engine.free();
      }
    }
    observations += 1;
  }
}
console.log(`conservative analysis OK: ${observations} TeX/Markdown cases; ${nativeOnly ? "native only (WASM not verified)" : "native/WASM parity"}`);

function nativeQueries(snapshot, queries) {
  const result = spawnSync("target/debug/semath-native", [], {
    encoding: "utf8",
    input: JSON.stringify({ snapshot, queries }),
    maxBuffer: 8 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

function assertParity(expected, actual, label) {
  const failure = firstDifferentialFailure([
    { name: "expected", value: expected },
    { name: "actual", value: actual },
  ]);
  if (failure) {
    throw new Error(`${label}: ${failure.path}; expected=${JSON.stringify(failure.expected)} actual=${JSON.stringify(failure.actual)}`);
  }
}

function verify(item, source, results, label) {
  const [semantic, diagnostics, definition, references, rename] = results.map((result) => result.value);
  assert.equal(semantic.kind, "semanticView", label);
  assert.equal(diagnostics.kind, "diagnostics", label);
  assert.deepEqual([...new Set(diagnostics.diagnostics.map((entry) => entry.code))].sort(), [...item.diagnostics].sort(), `${label}: diagnostics`);
  for (const diagnostic of diagnostics.diagnostics) {
    assert.ok(diagnostic.evidence.length > 0, `${label}: diagnostic evidence`);
    assert.ok(diagnostic.evidence.every((entry) => entry.sourceRanges.length > 0), `${label}: diagnostic source`);
  }
  if (item.unproved) {
    assert.ok(["partial", "unsupported"].includes(semantic.view.formulaAnalysis.disposition), `${label}: unexpected formula authority`);
    assert.ok(semantic.view.formulaAnalysis.interpretations.hypotheses.every((entry) => entry.label !== "red" && entry.label !== "blue"), `${label}: unrelated alternatives`);
  }
  if (item.noTypedRelation) {
    assert.ok(semantic.view.formulaAnalysis.interpretations.hypotheses.every((entry) => !entry.relation), `${label}: guessed relation`);
  }
  if (item.definition === true) {
    assert.equal(definition.locations.length, 1, `${label}: definition`);
    assert.equal(source.content.slice(definition.locations[0].range.startOffset, definition.locations[0].range.endOffset), item.symbol ?? "z", `${label}: definition source`);
    assert.equal(references.locations.length, 2, `${label}: complete references`);
  } else if (item.definition === false) {
    assert.deepEqual(definition.locations, [], `${label}: no guessed definition`);
  }
  if (item.definition === true && item.rename !== false) {
    assert.equal(rename.kind, "editProposal", `${label}: rename result`);
    assert.ok(rename.proposal, `${label}: authorized rename`);
    const edits = rename.proposal.files.flatMap((file) => file.edits);
    assert.equal(edits.length, 2, `${label}: complete rename`);
    for (const edit of edits) {
      assert.equal(source.content.slice(edit.range.startOffset, edit.range.endOffset), item.symbol ?? "z", `${label}: edit source`);
      assert.equal(edit.replacementText, "w", `${label}: replacement`);
    }
  } else if (item.definition === false || item.rename === false) {
    assert.equal(rename.kind, "editProposal", `${label}: rename refusal result`);
    assert.equal(rename.authorization.status, "refused", `${label}: rename refusal`);
    assert.ok(!rename.proposal, `${label}: no guessed rename`);
  }
}
