import { spawnSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  observeAuthoredScientificProbe,
  parseAuthoredScientificFixture,
  scoreAuthoredScientificFixture,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredScientificProbe,
  type AuthoredScientificSurfaceResults,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { semanticEvaluationCursorOffset } from "./semantic-evaluation-runner";

const fixtures = await Promise.all([
  readFixture(
    new URL(
      "../fixtures/challenge/document-reasoning-development-v1.json",
      import.meta.url,
    ),
  ),
  readFixture(
    new URL(
      "../fixtures/challenge/document-reasoning-holdout-v1.json",
      import.meta.url,
    ),
  ),
]);
const requestedSplit = process.env.SEMATH_AUTHORED_SPLIT;
if (
  requestedSplit !== undefined &&
  requestedSplit !== "development" &&
  requestedSplit !== "holdout"
) {
  throw new Error(
    "SEMATH_AUTHORED_SPLIT must be development or holdout",
  );
}
const selected = fixtures.filter(
  (fixture) =>
    requestedSplit === undefined || fixture.batch.split === requestedSplit,
);
buildNative();

const report = [];
let failures = 0;
for (const fixture of selected) {
  const observations: AuthoredScientificObservation[] = [];
  for (const [index, probe] of fixture.probes.entries()) {
    observations.push(runProbe(fixture, probe));
    if ((index + 1) % 10 === 0 || index + 1 === fixture.probes.length) {
      console.error(
        fixture.batch.split +
          ": " +
          (index + 1) +
          "/" +
          fixture.probes.length,
      );
    }
  }
  const score = scoreAuthoredScientificFixture(fixture, observations);
  failures += score.failures.length;
  report.push({
    batch: fixture.batch,
    observations,
    score,
  });
  console.log(
    fixture.batch.split +
      ": " +
      score.passed +
      "/" +
      score.cases +
      "; risk " +
      score.risk.total +
      " (false-establishment " +
      score.risk.falseEstablishment +
      ", false-conflict " +
      score.risk.falseConflict +
      ", identity " +
      score.risk.navigationOrIdentity +
      ", missed " +
      score.risk.missedCoverage +
      ")",
  );
}

if (process.env.SEMATH_AUTHORED_REPORT) {
  await writeFile(
    process.env.SEMATH_AUTHORED_REPORT,
    JSON.stringify({ results: report }, null, 2) + "\n",
  );
}
if (
  failures > 0 &&
  process.env.SEMATH_AUTHORED_ALLOW_FAILURES !== "1"
) {
  const messages = report.flatMap((item) => item.score.failures);
  throw new Error(
    "authored scientific evaluation failed:\n" + messages.join("\n"),
  );
}

async function readFixture(path: URL): Promise<AuthoredScientificFixture> {
  return parseAuthoredScientificFixture(
    JSON.parse(await readFile(path, "utf8")),
  );
}

function buildNative(): void {
  const build = spawnSync(
    "cargo",
    ["build", "--quiet", "--locked", "-p", "semath-native"],
    { encoding: "utf8" },
  );
  if (build.status !== 0) {
    throw new Error(build.stderr || "failed to build semath-native");
  }
}

function runProbe(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificProbe,
): AuthoredScientificObservation {
  const scenario = fixture.scenarios.find(
    (item) => item.id === probe.scenarioId,
  );
  const snapshot = scenario?.snapshots.find(
    (item) => item.id === probe.cursor.snapshotId,
  );
  if (!scenario || !snapshot) {
    throw new Error(probe.id + ": missing scenario snapshot");
  }
  const syntax = new LatexSyntaxService();
  const sources = snapshot.documents.map((document) => ({
    ...document,
    documentVersion: 1,
    language: languageOf(document.path),
  }));
  syntax.reset({ documents: sources });
  const documents: ProjectDocument[] = sources.map((source) => {
    const parsed = syntax.getFile(source.fileId);
    if (!parsed) {
      throw new Error(probe.id + ": missing neutral syntax");
    }
    return adaptWasmtexDocument({
      content: source.content,
      language: source.language,
      syntax: parsed,
    });
  });
  const cursorDocument = sources.find(
    (document) => document.fileId === probe.cursor.fileId,
  );
  if (!cursorDocument) {
    throw new Error(probe.id + ": unknown cursor file");
  }
  const offset = semanticEvaluationCursorOffset(
    cursorDocument.content,
    probe.cursor,
  );
  const epoch =
    "authored-v027-" + fixture.batch.split + "-" + probe.id;
  const target = {
    fileId: probe.cursor.fileId,
    offset,
  };
  const queries = [
    { ...target, kind: "semanticView" },
    { ...target, kind: "definition" },
    { ...target, kind: "references" },
    { ...target, kind: "prepareRename" },
    { ...target, kind: "rename", newName: "renamed" },
    { fileId: probe.cursor.fileId, kind: "diagnostics" },
  ] as const;
  const envelopes: QueryEnvelope[] = queries.map((query) => ({
    analysisGeneration: 0,
    documentVersion: 1,
    epoch,
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query,
  }));
  const native = spawnSync("target/debug/semath-native", [], {
    encoding: "utf8",
    input: JSON.stringify({
      queries: envelopes,
      snapshot: {
        documents,
        epoch,
        inventoryVersion: 1,
        mainFileId: probe.cursor.fileId,
        projectId: epoch,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      },
    }),
    maxBuffer: 16 * 1024 * 1024,
  });
  if (native.status !== 0) {
    throw new Error(
      probe.id + ": " + (native.stderr || "native evaluation failed"),
    );
  }
  const results = JSON.parse(native.stdout) as QueryResult[];
  if (results.length !== queries.length) {
    throw new Error(
      probe.id +
        ": native returned " +
        results.length +
        "/" +
        queries.length +
        " results",
    );
  }
  return observeAuthoredScientificProbe(
    probe,
    {
      semanticView: results[0],
      definition: results[1],
      references: results[2],
      prepareRename: results[3],
      rename: results[4],
      diagnostics: results[5],
    } as AuthoredScientificSurfaceResults,
  );
}

function languageOf(path: string): "latex" | "markdown" {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
