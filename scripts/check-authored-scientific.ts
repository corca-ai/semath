import { spawnSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  FIRST_LOSS_STAGES,
  evidenceGradedBreadthFailures,
  authoredProbeIdentityFailures,
  authoredRelationRangeMatches,
  authoredScenarioFor,
  authoredSnapshotFor,
  classifyAuthoredFirstLoss,
  frontierSignals,
  evaluateMathAuthoringDevelopment,
  observeAuthoredScientificProbe,
  parseAuthoredScientificFixture,
  parseFreshBlindReleaseFixture,
  resolveAuthoredAnchor,
  roleInstancesMatch,
  scoreAuthoredScientificFixture,
  summarizeEvidenceGradedHypotheses,
  summarizeAuthoredFirstLoss,
  type AuthoredRelationSourceEvidence,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredScientificProbe,
  type AuthoredScientificSurfaceResults,
  type RecognitionFrontierSignals,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
  type SourceRange,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { semanticEvaluationCursorOffset } from "./semantic-evaluation-runner";

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
const requestedFixture = process.env.SEMATH_AUTHORED_FIXTURE;
const fixturePaths = requestedFixture
  ? [new URL(requestedFixture, `file://${process.cwd()}/`)]
  : requestedSplit === "development"
    ? [
        new URL(
          "../fixtures/challenge/document-reasoning-development-v1.json",
          import.meta.url,
        ),
      ]
    : requestedSplit === "holdout"
      ? [
          new URL(
            "../fixtures/challenge/document-reasoning-holdout-v1.json",
            import.meta.url,
          ),
        ]
      : [
          new URL(
            "../fixtures/challenge/document-reasoning-development-v1.json",
            import.meta.url,
          ),
          new URL(
            "../fixtures/challenge/document-reasoning-holdout-v1.json",
            import.meta.url,
          ),
        ];
const selected = await Promise.all(fixturePaths.map(readFixture));
if (
  requestedSplit !== undefined &&
  selected.some((fixture) => fixture.batch.split !== requestedSplit)
) {
  throw new Error("SEMATH_AUTHORED_FIXTURE split does not match SEMATH_AUTHORED_SPLIT");
}
if (process.env.SEMATH_AUTHORED_SKIP_BUILD !== "1") buildNative();

const report = [];
let failures = 0;
for (const fixture of selected) {
  const runs: ProbeRun[] = [];
  for (const [index, probe] of fixture.probes.entries()) {
    runs.push(runProbe(fixture, probe));
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
  const observations = runs.map((run) => run.observation);
  const score = scoreAuthoredScientificFixture(fixture, observations);
  const authoringProbes = fixture.probes.flatMap((probe) => {
    const authoringContext = probe.expected.authoringContext;
    return authoringContext === undefined
      ? []
      : [{ expected: { authoringContext }, id: probe.id }];
  });
  const authoringProbeIds = new Set(authoringProbes.map((probe) => probe.id));
  const authoringEvaluation = evaluateMathAuthoringDevelopment(
    authoringProbes,
    observations
      .filter((observation) => authoringProbeIds.has(observation.caseId))
      .map((observation) => ({
        ...(observation.authoringContext === undefined
          ? {}
          : { authoringContext: observation.authoringContext }),
        caseId: observation.caseId,
      })),
  );
  const mathAuthoring = {
    ...authoringEvaluation,
    required: authoringProbes.length > 0,
  };
  const evidenceFacets = summarizeEvidenceGradedHypotheses(observations);
  const evidenceGraded = {
    ...evidenceFacets,
    failures: [
      ...evidenceFacets.failures,
      ...evidenceGradedBreadthFailures(evidenceFacets),
    ],
  };
  const failedIds = new Set(
    score.failures.map((failure) => failure.slice(0, failure.indexOf(":"))),
  );
  const firstLoss = runs.map((run, index) => {
    const probe = fixture.probes[index];
    if (!probe) throw new Error(`missing probe for observation ${index}`);
    const scenario = authoredScenarioFor(fixture, probe);
    return {
      caseId: probe.id,
      expectedDecision: probe.expected.decision,
      family: probe.family,
      field: scenario.field,
      split: fixture.batch.split,
      ...classifyAuthoredFirstLoss({
        cursorSignals: run.cursorSignals,
        expectedDecision: probe.expected.decision,
        expectedRelationsMatched: expectedRelationsMatch(
          fixture,
          probe,
          run.observation,
        ),
        identityFailures: authoredProbeIdentityFailures(
          fixture,
          probe,
          run.observation,
        ),
        probePassed: !failedIds.has(probe.id),
        relationSources: run.relationSources,
      }),
      cursorSignals: run.cursorSignals,
      relationSources: run.relationSources,
    };
  });
  const firstLossCounts = Object.fromEntries(
    FIRST_LOSS_STAGES.map((stage) => [
      stage,
      firstLoss.filter((item) => item.stage === stage).length,
    ]),
  );
  const firstLossAtlas = summarizeAuthoredFirstLoss(firstLoss);
  failures += score.failures.length + evidenceGraded.failures.length +
    mathAuthoring.failures.length;
  report.push({
    batch: fixture.batch,
    firstLoss,
    firstLossAtlas,
    firstLossCounts,
    observations,
    evidenceGraded,
    mathAuthoring,
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
      "); first loss " +
      Object.entries(firstLossCounts)
        .filter(([, count]) => count > 0)
        .map(([stage, count]) => stage + " " + count)
        .join(", "),
  );
  console.log(
    fixture.batch.split +
      " math authoring: " +
      mathAuthoring.exactCases +
      "/" +
      mathAuthoring.cases +
      " exact; required " +
      mathAuthoring.required,
  );
  console.log(
    fixture.batch.split +
      " evidence facets: hypotheses " +
      evidenceGraded.withHypotheses +
      "/" +
      evidenceGraded.cases +
      ", multiple " +
      evidenceGraded.multipleHypothesisCases +
      ", support " +
      evidenceGraded.supportingEvidenceCases +
      ", contradiction " +
      evidenceGraded.contradictionCases +
      ", discriminators " +
      evidenceGraded.missingDiscriminatorCases +
      ", natural-language " +
      evidenceGraded.naturalLanguageCases +
      ", domain " +
      evidenceGraded.domainContextCases +
      ", convention " +
      evidenceGraded.reviewedConventionCases +
      ", open-world " +
      evidenceGraded.openWorldCases +
      ", exact-anchor " +
      evidenceGraded.exactAnchorCases +
      ", ordering " +
      evidenceGraded.orderingCases +
      "; safety failures " +
      evidenceGraded.failures.length,
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
  const messages = report.flatMap((item) => [
    ...item.score.failures,
    ...item.evidenceGraded.failures,
    ...item.mathAuthoring.failures,
  ]);
  throw new Error(
    "authored scientific evaluation failed:\n" + messages.join("\n"),
  );
}

async function readFixture(path: URL): Promise<AuthoredScientificFixture> {
  const value = JSON.parse(await readFile(path, "utf8")) as unknown;
  if (
    typeof value === "object" &&
    value !== null &&
    "commissioning" in value &&
    "release" in value
  ) {
    return parseFreshBlindReleaseFixture(value).fixture;
  }
  return parseAuthoredScientificFixture(value);
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
): ProbeRun {
  const scenario = authoredScenarioFor(fixture, probe);
  const snapshot = authoredSnapshotFor(scenario, probe);
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
  const relationTargets = probe.expected.relations.map((relation) => {
    const anchor = resolveAuthoredAnchor(snapshot, relation.anchor);
    const document = documents.find((item) => item.fileId === anchor.fileId);
    if (!document) throw new Error(probe.id + ": missing relation document");
    const mathRoot = document.mathRoots.find((root) =>
      rangesOverlap(root.contentRange, anchor.range),
    );
    return {
      anchor,
      // Query the reviewed relation at its trailing edge. Leading equation
      // metadata (for example `\\label`) is presentation, and querying its
      // start would misclassify an already recognized formula as an
      // attachment loss.
      offset: mathRoot
        ? Math.min(mathRoot.contentRange.endOffset, anchor.range.endOffset)
        : anchor.range.endOffset,
      relation,
      syntaxAvailable: Boolean(mathRoot),
    };
  });
  const queries = [
    { ...target, kind: "semanticView" },
    { ...target, kind: "definition" },
    { ...target, kind: "references", includeDeclaration: true },
    { ...target, kind: "prepareRename" },
    {
      ...target,
      kind: "rename",
      newName: probe.expected.navigation.rename.newName ?? "renamed",
    },
    { fileId: probe.cursor.fileId, kind: "diagnostics" },
    ...relationTargets.map((source) => ({
      fileId: source.anchor.fileId,
      kind: "semanticView" as const,
      offset: source.offset,
    })),
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
  const observation = observeAuthoredScientificProbe(
    probe,
    {
      semanticView: results[0],
      definition: results[1],
      references: results[2],
      prepareRename: results[3],
      rename: results[4],
      diagnostics: results[5],
      relationViews: relationTargets.map((source, index) => ({
        fileId: source.anchor.fileId,
        result: results[6 + index]!,
      })),
    } as AuthoredScientificSurfaceResults,
  );
  const cursorView = semanticView(results[0], probe.id + ": cursor");
  const cursorDocumentSyntax = documents.find(
    (document) => document.fileId === probe.cursor.fileId,
  );
  const cursorSignals = frontierSignals(
    cursorView,
    Boolean(
      cursorDocumentSyntax?.mathRoots.some(
        (root) =>
          root.contentRange.startOffset <= offset &&
          offset <= root.contentRange.endOffset,
      ),
    ),
  );
  const relationSources = relationTargets.map(
    (source, index): AuthoredRelationSourceEvidence => {
      const view = semanticView(
        results[6 + index],
        probe.id + ": relation source " + index,
      );
      const documentContent =
        documents.find((document) => document.fileId === source.anchor.fileId)
          ?.content ?? "";
      const sameRelation = view.context.relations.filter(
        (relation) => relation.relationId === source.relation.relationId,
      );
      const sameRange = sameRelation.filter((relation) =>
        authoredRelationRangeMatches(
          documentContent,
          relation.range,
          source.anchor.range,
          relation.evidence.find(
            (evidence) => evidence.ruleId === "semantic-law-unification",
          )?.sourceRanges[0],
        ),
      );
      const rolesMatched = sameRange.some((relation) =>
        roleInstancesMatch(relation.roles, source.relation.roles, undefined),
      );
      return {
        localRelationMatched: rolesMatched,
        rangeMatched: sameRange.length > 0,
        relationId: source.relation.relationId,
        relationPresent: sameRelation.length > 0,
        rolesMatched,
        signals: frontierSignals(view, source.syntaxAvailable),
      };
    },
  );
  return { cursorSignals, observation, relationSources };
}

interface ProbeRun {
  readonly cursorSignals: RecognitionFrontierSignals;
  readonly observation: AuthoredScientificObservation;
  readonly relationSources: readonly AuthoredRelationSourceEvidence[];
}

function semanticView(
  result: QueryResult | undefined,
  context: string,
) {
  if (!result || result.value.kind !== "semanticView") {
    throw new Error(context + ": semanticView result is unavailable");
  }
  return result.value.view;
}

function expectedRelationsMatch(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificProbe,
  observation: AuthoredScientificObservation,
): boolean {
  const snapshot = authoredSnapshotFor(authoredScenarioFor(fixture, probe), probe);
  return probe.expected.relations.every((expected) => {
    const anchor = resolveAuthoredAnchor(snapshot, expected.anchor);
    const document = snapshot.documents.find(
      (document) => document.fileId === anchor.fileId,
    );
    return observation.relations.some(
      (relation) =>
        relation.relationId === expected.relationId &&
        relation.fileId === anchor.fileId &&
        document !== undefined &&
        authoredRelationRangeMatches(
          document.content,
          relation.range,
          anchor.range,
          relation.formulaRange,
        ) &&
        relation.sourceGrounded === expected.sourceGrounded &&
        roleInstancesMatch(relation.roles, expected.roles, undefined),
    );
  });
}

function rangesOverlap(left: SourceRange, right: SourceRange): boolean {
  return left.startOffset < right.endOffset && right.startOffset < left.endOffset;
}

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return left.startOffset === right.startOffset && left.endOffset === right.endOffset;
}

function languageOf(path: string): "latex" | "markdown" {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
