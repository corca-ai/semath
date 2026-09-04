import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import {
  freshBlindAuthoringSafetySummary,
  freshBlindSafetyGateFailed,
  freshBlindSafetySummary,
} from "../packages/evaluation/src/fresh-blind-release";
import type {
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "../packages/evaluation/src/index";
import {
  evaluateMathAuthoringDevelopment,
  parseObservedMathAuthoringContext,
  type MathAuthoringContextFailure,
  type MathAuthoringExpectationProbe,
  type MathAuthoringFailureKind,
} from "../packages/evaluation/src/index";
import {
  checkFreshBlindReservationIdentity,
  type FreshBlindReservation,
} from "./check-fresh-blind-reservation";
import { loadFreshBlindEvidence, sha256 } from "./fresh-blind-evidence";
import {
  createFreshBlindStartedReceipt,
  finalizeFreshBlindReceipt,
  reserveFreshBlindReceipt,
  type FreshBlindStartedReceipt,
  type FreshBlindTerminalReceipt,
} from "./fresh-blind-receipt";
import {
  parseFreshBlindPreflightManifest,
  type FreshBlindPreflightManifest,
} from "./fresh-blind-preflight-manifest";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

if (import.meta.main) await runFreshBlindRelease();

async function runFreshBlindRelease(): Promise<void> {
  assertFreshBlindLinuxX64();
  const fixturePath = requiredPath("SEMATH_FRESH_BLIND_FIXTURE");
  const receiptPath = requiredPath("SEMATH_FRESH_BLIND_RECEIPT");
  const releaseId = required("SEMATH_RELEASE_ID");
  const candidateSha = required("SEMATH_CANDIDATE_SHA");
  assertFreshBlindWorkflowBoundary(
    freshBlindWorkflowBoundaryFromEnvironment(candidateSha),
  );
  const reservationPath = requiredPath("SEMATH_FRESH_BLIND_RESERVATION");
  const manifestPath = requiredPath("SEMATH_FRESH_BLIND_PREFLIGHT_MANIFEST");

  // Everything before reserveFreshBlindReceipt is static validation or
  // compilation. The semantic engine has not seen the fresh fixture yet.
  const evidence = await loadFreshBlindEvidence(fixturePath);
  if (evidence.release.release.id !== releaseId)
    throw new Error("fresh blind fixture id does not match SEMATH_RELEASE_ID");
  assertCandidate(candidateSha);
  const manifestBytes = await readFile(manifestPath);
  const manifest = parseFreshBlindPreflightManifest(
    JSON.parse(manifestBytes.toString("utf8")) as unknown,
  );
  assertManifestExecution(manifest, {
    candidateSha,
    fixtureId: evidence.release.release.id,
    fixtureSeal: evidence.release.release.seal,
    fixtureSha256: sha256(await readFile(fixturePath)),
  });
  const reservationBytes = await readFile(reservationPath);
  const reservation = await checkFreshBlindReservationIdentity({
    candidateSha,
    fixturePath,
    releaseId,
    reservationPath,
    runAttempt: required("GITHUB_RUN_ATTEMPT"),
    runId: required("GITHUB_RUN_ID"),
  });
  const nativeSha256 = sha256(await readFile("target/debug/semath-native"));
  if (nativeSha256 !== manifest.artifacts.nativeSha256)
    throw new Error(
      "execution native binary differs from the pre-blind manifest",
    );
  assertCandidate(candidateSha);

  const started = createFreshBlindStartedReceipt({
    manifest,
    manifestSha256: sha256(manifestBytes),
    reservation,
    reservationSha256: sha256(reservationBytes),
    startedAt: new Date().toISOString(),
  });
  const artifactDirectory = dirname(receiptPath);
  const evaluationPath = join(artifactDirectory, "evaluation.json");
  const lifecyclePath = join(artifactDirectory, "lifecycle.json");
  let receiptReserved = false;
  let terminalReceiptWritten = false;
  let evaluation:
    | { readonly bytes: Uint8Array; readonly parsed: ParsedEvaluation }
    | undefined;
  try {
    await reserveFreshBlindReceipt(receiptPath, started);
    receiptReserved = true;
    run("bun", ["scripts/check-authored-scientific.ts"], {
      SEMATH_AUTHORED_ALLOW_FAILURES: "1",
      SEMATH_AUTHORED_FIXTURE: evidence.path,
      SEMATH_AUTHORED_REPORT: evaluationPath,
      SEMATH_AUTHORED_SPLIT: "holdout",
      SEMATH_AUTHORED_SKIP_BUILD: "1",
    });
    const evaluationBytes = await readFile(evaluationPath);
    evaluation = {
      bytes: evaluationBytes,
      parsed: parseFreshBlindEvaluation(
        JSON.parse(evaluationBytes.toString("utf8")) as unknown,
        evidence.release.fixture.probes.flatMap((probe) =>
          probe.expected.authoringContext === undefined
            ? []
            : [{
                expected: { authoringContext: probe.expected.authoringContext },
                id: probe.id,
              }]
        ),
        {
          cursorSurfaceIdentityRequired: evidence.release.schemaVersion === 3,
          formulaDecisionRequired: evidence.release.schemaVersion === 3,
          surfaceAuthorizationsRequired: evidence.release.schemaVersion === 3,
        },
      ),
    };
    run("bun", ["scripts/check-fresh-blind-lifecycle.ts"], {
      SEMATH_FRESH_BLIND_FIXTURE: evidence.path,
      SEMATH_FRESH_BLIND_LIFECYCLE_REPORT: lifecyclePath,
    });
    const lifecycleBytes = await readFile(lifecyclePath);
    const lifecycle = parseFreshBlindLifecycle(
      JSON.parse(lifecycleBytes.toString("utf8")) as unknown,
    );
    if (
      lifecycle.fixtureId !== releaseId ||
      lifecycle.fixtureSeal !== evidence.release.release.seal
    )
      throw new Error("lifecycle report does not match the reserved fixture");
    const safety = freshBlindSafetySummary(
      evidence.release.fixture,
      evaluation.parsed.observations,
    );
    const authoringSafety = freshBlindAuthoringSafetySummary(
      evidence.release,
      evaluation.parsed.observations,
    );
    const facetFailures = [
      ...evaluation.parsed.evidenceGradedFailures,
      ...authoringSafety.failures.map(
        (failure) => `${failure.path}: ${failure.kind}`,
      ),
    ];
    if (evaluation.parsed.mathAuthoringRequired) {
      facetFailures.push(...evaluation.parsed.mathAuthoringFailures);
      if (evaluation.parsed.mathAuthoringCases !==
        evaluation.parsed.mathAuthoringExactCases) {
        facetFailures.push(
          `exact authoring context ${evaluation.parsed.mathAuthoringExactCases}/${evaluation.parsed.mathAuthoringCases}`,
        );
      }
    }
    const safetyFailed =
      freshBlindSafetyGateFailed(safety) || facetFailures.length > 0;
    const completed = terminalReceipt(started, {
      evaluationSha256: sha256(evaluationBytes),
      lifecycleSha256: sha256(lifecycleBytes),
      result: {
        authoringSafety,
        evaluation: evaluation.parsed.raw,
        facetFailureIds: facetFailures,
        lifecycle,
        safety,
        validation: evidence.summary,
      },
      status: safetyFailed ? "safety-failed" : "completed",
    });
    await finalizeFreshBlindReceipt(receiptPath, completed);
    terminalReceiptWritten = true;
    if (safetyFailed)
      throw new Error(
        "fresh blind safety gate failed; full evidence remains in the terminal receipt",
      );
    console.log(
      `fresh blind release recorded: ${evaluation.parsed.score.passed}/${evaluation.parsed.score.cases}; receipt ${receiptPath}`,
    );
  } catch (error) {
    if (receiptReserved && !terminalReceiptWritten) {
      const failed = terminalReceipt(started, {
        evaluationSha256: evaluation ? sha256(evaluation.bytes) : null,
        lifecycleSha256: null,
        result: {
          error: error instanceof Error ? error.message : String(error),
          evaluation: evaluation?.parsed.raw ?? null,
        },
        status: "execution-error",
      });
      await finalizeFreshBlindReceipt(receiptPath, failed);
    }
    throw error;
  }
}

interface ParsedEvaluation {
  readonly evidenceGradedFailures: readonly string[];
  readonly mathAuthoringCases: number;
  readonly mathAuthoringExactCases: number;
  readonly mathAuthoringFailures: readonly string[];
  readonly mathAuthoringRequired: boolean;
  readonly observations: readonly AuthoredScientificObservation[];
  readonly raw: unknown;
  readonly score: AuthoredScientificScorecard;
}

export function parseFreshBlindEvaluation(
  value: unknown,
  expectedProbes: readonly MathAuthoringExpectationProbe[],
  options: {
    readonly cursorSurfaceIdentityRequired?: boolean;
    readonly formulaDecisionRequired?: boolean;
    readonly surfaceAuthorizationsRequired?: boolean;
  } = {},
): ParsedEvaluation {
  const report = record(value, "fresh blind evaluation");
  exact(report, ["results"], "fresh blind evaluation");
  if (!Array.isArray(report.results) || report.results.length !== 1)
    throw new Error("fresh blind evaluation must contain exactly one result");
  const result = record(report.results[0], "fresh blind evaluation.results[0]");
  exact(
    result,
    [
      "batch",
      "evidenceGraded",
      "firstLoss",
      "firstLossAtlas",
      "firstLossCounts",
      "mathAuthoring",
      "observations",
      "score",
    ],
    "fresh blind evaluation.results[0]",
  );
  if (!Array.isArray(result.observations))
    throw new Error("fresh blind evaluation observations must be an array");
  const observations = result.observations.map((item, index) =>
    parseObservation(
      item,
      `fresh blind evaluation observations[${index}]`,
      options.cursorSurfaceIdentityRequired === true,
      options.formulaDecisionRequired === true,
      options.surfaceAuthorizationsRequired === true,
    ),
  );
  if (
    new Set(observations.map((item) => item.caseId)).size !==
    observations.length
  ) {
    throw new Error("fresh blind evaluation observation ids must be unique");
  }
  const score = parseScore(result.score);
  const evidenceGraded = record(
    result.evidenceGraded,
    "fresh blind evaluation evidenceGraded",
  );
  exact(
    evidenceGraded,
    [
      "cases",
      "contradictionCases",
      "domainContextCases",
      "exactAnchorCases",
      "failures",
      "missingDiscriminatorCases",
      "multipleHypothesisCases",
      "naturalLanguageCases",
      "openWorldCases",
      "orderingCases",
      "reviewedConventionCases",
      "supportingEvidenceCases",
      "withHypotheses",
    ],
    "fresh blind evaluation evidenceGraded",
  );
  const evidenceCases = integer(
    evidenceGraded.cases,
    "fresh blind evaluation evidenceGraded.cases",
  );
  for (const key of [
    "contradictionCases",
    "domainContextCases",
    "exactAnchorCases",
    "missingDiscriminatorCases",
    "multipleHypothesisCases",
    "naturalLanguageCases",
    "openWorldCases",
    "orderingCases",
    "reviewedConventionCases",
    "supportingEvidenceCases",
    "withHypotheses",
  ] as const) {
    const count = integer(
      evidenceGraded[key],
      `fresh blind evaluation evidenceGraded.${key}`,
    );
    if (count > evidenceCases)
      throw new Error(
        `fresh blind evaluation evidenceGraded.${key} exceeds cases`,
      );
  }
  const mathAuthoring = record(
    result.mathAuthoring,
    "fresh blind evaluation mathAuthoring",
  );
  exact(
    mathAuthoring,
    ["cases", "exactCases", "failures", "findings", "required"],
    "fresh blind evaluation mathAuthoring",
  );
  const mathAuthoringRequired = expectedProbes.length > 0;
  if (mathAuthoring.required !== mathAuthoringRequired) {
    throw new Error(
      `fresh blind evaluation mathAuthoring.required must be ${mathAuthoringRequired}`,
    );
  }
  const mathAuthoringCases = integer(
    mathAuthoring.cases,
    "fresh blind evaluation mathAuthoring.cases",
  );
  const mathAuthoringExactCases = integer(
    mathAuthoring.exactCases,
    "fresh blind evaluation mathAuthoring.exactCases",
  );
  const mathAuthoringFailures = strings(
    mathAuthoring.failures,
    "fresh blind evaluation mathAuthoring.failures",
  );
  const mathAuthoringFindings = parseMathAuthoringFindings(
    mathAuthoring.findings,
    "fresh blind evaluation mathAuthoring.findings",
  );
  if (mathAuthoringRequired && mathAuthoringCases === 0)
    throw new Error("fresh blind evaluation mathAuthoring.cases must be positive");
  if (!mathAuthoringRequired &&
    (mathAuthoringCases !== 0 || mathAuthoringExactCases !== 0 ||
      mathAuthoringFailures.length > 0 || mathAuthoringFindings.length > 0)) {
    throw new Error(
      "fresh blind evaluation non-required mathAuthoring must be 0/0 with no findings",
    );
  }
  if (mathAuthoringExactCases > mathAuthoringCases)
    throw new Error(
      "fresh blind evaluation mathAuthoring.exactCases exceeds cases",
    );
  if (!("firstLossAtlas" in result))
    throw new Error("fresh blind evaluation is missing firstLossAtlas");
  if (
    score.cases !== observations.length ||
    evidenceCases !== score.cases ||
    (mathAuthoringRequired
      ? mathAuthoringCases !== score.cases
      : mathAuthoringCases !== 0)
  ) {
    throw new Error("fresh blind evaluation case counts disagree");
  }
  if (score.passed > score.cases)
    throw new Error("fresh blind evaluation passed exceeds cases");
  const expectedIds = new Set(expectedProbes.map((probe) => probe.id));
  const recomputed = evaluateMathAuthoringDevelopment(
    expectedProbes,
    observations
      .filter((observation) => expectedIds.has(observation.caseId))
      .map((observation) => ({
        ...(observation.authoringContext === undefined
          ? {}
          : { authoringContext: observation.authoringContext }),
        caseId: observation.caseId,
      })),
  );
  if (recomputed.cases !== mathAuthoringCases ||
    recomputed.exactCases !== mathAuthoringExactCases ||
    !isDeepStrictEqual(recomputed.failures, mathAuthoringFailures) ||
    !isDeepStrictEqual(recomputed.findings, mathAuthoringFindings)) {
    throw new Error(
      "fresh blind evaluation mathAuthoring does not match independent recomputation",
    );
  }
  return {
    evidenceGradedFailures: strings(
      evidenceGraded.failures,
      "fresh blind evaluation evidenceGraded.failures",
    ),
    mathAuthoringCases,
    mathAuthoringExactCases,
    mathAuthoringFailures,
    mathAuthoringRequired,
    observations,
    raw: value,
    score,
  };
}

export interface FreshBlindLifecycleReport {
  readonly comparedProbes: number;
  readonly comparedStages: number;
  readonly fixtureId: string;
  readonly fixtureSeal: string;
  readonly schemaVersion: 1;
}

export function parseFreshBlindLifecycle(
  value: unknown,
): FreshBlindLifecycleReport {
  const item = record(value, "fresh blind lifecycle");
  exact(
    item,
    [
      "comparedProbes",
      "comparedStages",
      "fixtureId",
      "fixtureSeal",
      "schemaVersion",
    ],
    "fresh blind lifecycle",
  );
  if (item.schemaVersion !== 1)
    throw new Error("fresh blind lifecycle schemaVersion must be 1");
  return {
    comparedProbes: integer(
      item.comparedProbes,
      "fresh blind lifecycle.comparedProbes",
    ),
    comparedStages: integer(
      item.comparedStages,
      "fresh blind lifecycle.comparedStages",
    ),
    fixtureId: checked(
      item.fixtureId,
      /^v0\.[1-9][0-9]*$/u,
      "fresh blind lifecycle.fixtureId",
    ),
    fixtureSeal: checked(
      item.fixtureSeal,
      /^[0-9a-f]{64}$/u,
      "fresh blind lifecycle.fixtureSeal",
    ),
    schemaVersion: 1,
  };
}

function terminalReceipt(
  started: FreshBlindStartedReceipt,
  input: {
    readonly evaluationSha256: string | null;
    readonly lifecycleSha256: string | null;
    readonly result: unknown;
    readonly status: FreshBlindTerminalReceipt["status"];
  },
): FreshBlindTerminalReceipt {
  return {
    ...started,
    artifacts: {
      ...started.artifacts,
      evaluationSha256: input.evaluationSha256,
      lifecycleSha256: input.lifecycleSha256,
    },
    completedAt: new Date().toISOString(),
    result: input.result,
    status: input.status,
  };
}

function assertManifestExecution(
  manifest: FreshBlindPreflightManifest,
  identity: {
    readonly candidateSha: string;
    readonly fixtureId: string;
    readonly fixtureSeal: string;
    readonly fixtureSha256: string;
  },
): void {
  if (manifest.provenance.candidateCommit !== identity.candidateSha)
    throw new Error("pre-blind manifest candidate differs from execution");
  if (
    manifest.release.fixtureId !== identity.fixtureId ||
    manifest.release.fixtureSeal !== identity.fixtureSeal ||
    manifest.release.fixtureSha256 !== identity.fixtureSha256
  )
    throw new Error("pre-blind manifest fixture differs from execution");
}

function assertCandidate(candidateSha: string): void {
  if (output("git", ["rev-parse", "HEAD"]) !== candidateSha)
    throw new Error("fresh blind candidate SHA does not match HEAD");
  if (output("git", ["status", "--porcelain"]))
    throw new Error("fresh blind execution requires a clean worktree");
}

function parseScore(value: unknown): AuthoredScientificScorecard {
  const score = record(value, "fresh blind evaluation score");
  exact(
    score,
    ["cases", "failures", "passed", "risk"],
    "fresh blind evaluation score",
  );
  const risk = record(score.risk, "fresh blind evaluation score.risk");
  exact(
    risk,
    [
      "falseConflict",
      "falseEstablishment",
      "missedCoverage",
      "navigationOrIdentity",
      "total",
    ],
    "fresh blind evaluation score.risk",
  );
  const parsed = {
    cases: integer(score.cases, "fresh blind evaluation score.cases"),
    failures: strings(score.failures, "fresh blind evaluation score.failures"),
    passed: integer(score.passed, "fresh blind evaluation score.passed"),
    risk: {
      falseConflict: integer(
        risk.falseConflict,
        "fresh blind evaluation score.risk.falseConflict",
      ),
      falseEstablishment: integer(
        risk.falseEstablishment,
        "fresh blind evaluation score.risk.falseEstablishment",
      ),
      missedCoverage: integer(
        risk.missedCoverage,
        "fresh blind evaluation score.risk.missedCoverage",
      ),
      navigationOrIdentity: integer(
        risk.navigationOrIdentity,
        "fresh blind evaluation score.risk.navigationOrIdentity",
      ),
      total: integer(risk.total, "fresh blind evaluation score.risk.total"),
    },
  };
  for (const count of [
    parsed.risk.falseConflict,
    parsed.risk.falseEstablishment,
    parsed.risk.missedCoverage,
    parsed.risk.navigationOrIdentity,
  ]) {
    if (count > parsed.cases) {
      throw new Error("fresh blind evaluation risk count exceeds cases");
    }
  }
  const expectedTotal =
    parsed.risk.falseConflict * 12 +
    parsed.risk.falseEstablishment * 12 +
    parsed.risk.navigationOrIdentity * 10 +
    parsed.risk.missedCoverage * 2;
  if (parsed.risk.total !== expectedTotal) {
    throw new Error("fresh blind evaluation risk total is inconsistent");
  }
  return parsed;
}

function parseObservation(
  value: unknown,
  label: string,
  cursorSurfaceIdentityRequired: boolean,
  formulaDecisionRequired: boolean,
  surfaceAuthorizationsRequired: boolean,
): AuthoredScientificObservation {
  const item = record(value, label);
  const requiredKeys = [
    "authoringContext",
    "caseId",
    ...(cursorSurfaceIdentityRequired ? ["cursorSurfaceIdentity"] : []),
    "decision",
    "definitions",
    "diagnostics",
    ...(formulaDecisionRequired ? ["formulaDecision"] : []),
    ...(surfaceAuthorizationsRequired ? ["surfaceAuthorizations"] : []),
    "prepareRename",
    "proofGrounded",
    "references",
    "relations",
    "renameEdits",
    "symbol",
    "interpretations",
  ];
  const allowedKeys = new Set([
    ...requiredKeys,
    "cursorSurfaceIdentity",
    "renameSafety",
    "symbolLocation",
    "surfaceAuthorizations",
  ]);
  const missing = requiredKeys.filter((key) => !(key in item));
  const unknown = Object.keys(item).filter((key) => !allowedKeys.has(key));
  if (formulaDecisionRequired && !("formulaDecision" in item)) {
    throw new Error(`${label}.formulaDecision is required`);
  }
  if (cursorSurfaceIdentityRequired && !("cursorSurfaceIdentity" in item)) {
    throw new Error(`${label}.cursorSurfaceIdentity is required`);
  }
  if (surfaceAuthorizationsRequired && !("surfaceAuthorizations" in item)) {
    throw new Error(`${label}.surfaceAuthorizations is required`);
  }
  if (missing.length || unknown.length)
    throw new Error(`${label} has unexpected or missing fields`);
  const caseId = nonemptyString(item.caseId, `${label}.caseId`);
  const decision = checked(
    item.decision,
    /^(?:established|partial|ambiguous|conflicting|unsupported)$/u,
    `${label}.decision`,
  ) as AuthoredScientificObservation["decision"];
  const definitions = locations(item.definitions, `${label}.definitions`);
  const references = locations(item.references, `${label}.references`);
  const diagnostics = array(item.diagnostics, `${label}.diagnostics`).map(
    (value, index) => {
      const diagnostic = record(value, `${label}.diagnostics[${index}]`);
      exact(
        diagnostic,
        ["code", "fileId", "range", "severity"],
        `${label}.diagnostics[${index}]`,
      );
      return {
        code: nonemptyString(
          diagnostic.code,
          `${label}.diagnostics[${index}].code`,
        ),
        fileId: nonemptyString(
          diagnostic.fileId,
          `${label}.diagnostics[${index}].fileId`,
        ),
        range: sourceRange(
          diagnostic.range,
          `${label}.diagnostics[${index}].range`,
        ),
        severity: checked(
          diagnostic.severity,
          /^(?:error|hint|warning)$/u,
          `${label}.diagnostics[${index}].severity`,
        ) as "error" | "hint" | "warning",
      };
    },
  );
  const preparation = record(item.prepareRename, `${label}.prepareRename`);
  const preparationKeys = Object.keys(preparation).sort();
  if (preparationKeys.some((key) => key !== "placeholder" && key !== "range"))
    throw new Error(`${label}.prepareRename has unexpected fields`);
  const prepareRename = {
    ...(preparation.placeholder === undefined
      ? {}
      : {
          placeholder: nonemptyString(
            preparation.placeholder,
            `${label}.prepareRename.placeholder`,
          ),
        }),
    ...(preparation.range === undefined
      ? {}
      : {
          range: sourceRange(preparation.range, `${label}.prepareRename.range`),
        }),
  };
  if (typeof item.proofGrounded !== "boolean")
    throw new Error(`${label}.proofGrounded must be boolean`);
  const relations = array(item.relations, `${label}.relations`).map(
    (value, index) => {
      const relation = record(value, `${label}.relations[${index}]`);
      const keys = Object.keys(relation);
      const required = [
        "fileId",
        "relationId",
        "range",
        "roles",
        "sourceGrounded",
      ];
      if (
        required.some((key) => !(key in relation)) ||
        keys.some((key) => ![...required, "formulaRange"].includes(key))
      )
        throw new Error(
          `${label}.relations[${index}] has unexpected or missing fields`,
        );
      if (typeof relation.sourceGrounded !== "boolean")
        throw new Error(
          `${label}.relations[${index}].sourceGrounded must be boolean`,
        );
      return {
        fileId: nonemptyString(
          relation.fileId,
          `${label}.relations[${index}].fileId`,
        ),
        relationId: nonemptyString(
          relation.relationId,
          `${label}.relations[${index}].relationId`,
        ),
        range: sourceRange(
          relation.range,
          `${label}.relations[${index}].range`,
        ),
        ...(relation.formulaRange === undefined
          ? {}
          : {
              formulaRange: sourceRange(
                relation.formulaRange,
                `${label}.relations[${index}].formulaRange`,
              ),
            }),
        roles: array(relation.roles, `${label}.relations[${index}].roles`).map(
          (value, roleIndex) => {
            const role = record(
              value,
              `${label}.relations[${index}].roles[${roleIndex}]`,
            );
            const allowed = new Set(["conceptId", "role", "symbol"]);
            if (
              !("role" in role) ||
              !("symbol" in role) ||
              Object.keys(role).some((key) => !allowed.has(key))
            ) {
              throw new Error(
                `${label}.relations[${index}].roles[${roleIndex}] has unexpected or missing fields`,
              );
            }
            return {
              ...(role.conceptId === undefined
                ? {}
                : {
                    conceptId: nonemptyString(
                      role.conceptId,
                      `${label}.relations[${index}].roles[${roleIndex}].conceptId`,
                    ),
                  }),
              role: nonemptyString(
                role.role,
                `${label}.relations[${index}].roles[${roleIndex}].role`,
              ),
              symbol: nonemptyString(
                role.symbol,
                `${label}.relations[${index}].roles[${roleIndex}].symbol`,
              ),
            };
          },
        ),
        sourceGrounded: relation.sourceGrounded,
      };
    },
  );
  const renameEdits = array(item.renameEdits, `${label}.renameEdits`).map(
    (value, index) => {
      const edit = record(value, `${label}.renameEdits[${index}]`);
      exact(
        edit,
        ["expectedText", "fileId", "path", "range", "replacementText"],
        `${label}.renameEdits[${index}]`,
      );
      return {
        expectedText: string(
          edit.expectedText,
          `${label}.renameEdits[${index}].expectedText`,
        ),
        fileId: nonemptyString(
          edit.fileId,
          `${label}.renameEdits[${index}].fileId`,
        ),
        path: nonemptyString(edit.path, `${label}.renameEdits[${index}].path`),
        range: sourceRange(edit.range, `${label}.renameEdits[${index}].range`),
        replacementText: string(
          edit.replacementText,
          `${label}.renameEdits[${index}].replacementText`,
        ),
      };
    },
  );
  const authoringContext = item.authoringContext === undefined
    ? undefined
    : parseObservedMathAuthoringContext(
        item.authoringContext,
        `${label}.authoringContext`,
      );
  const formulaDecision = item.formulaDecision === undefined
    ? undefined
    : parseObservedFormulaDecision(item.formulaDecision, `${label}.formulaDecision`);
  if (
    formulaDecision &&
    (!authoringContext ||
      formulaDecision.status !== authoringContext.disposition ||
      !isDeepStrictEqual(
        formulaDecision.location,
        authoringContext.formula?.location ?? null,
      ))
  ) {
    throw new Error(`${label}.formulaDecision must match authoringContext`);
  }
  if (item.interpretations !== undefined)
    record(item.interpretations, `${label}.interpretations`);
  if (item.interpretations !== undefined &&
    (!authoringContext ||
      !isDeepStrictEqual(item.interpretations, authoringContext.interpretations))) {
    throw new Error(`${label}.interpretations must match authoringContext`);
  }
  const symbol =
    item.symbol === null
      ? null
      : nonemptyString(item.symbol, `${label}.symbol`);
  const cursorSurfaceIdentity = item.cursorSurfaceIdentity === undefined
    ? undefined
    : parseCursorSurfaceIdentity(
        item.cursorSurfaceIdentity,
        `${label}.cursorSurfaceIdentity`,
      );
  if (
    cursorSurfaceIdentity !== undefined &&
    (symbol === null) !== (cursorSurfaceIdentity === null)
  ) {
    throw new Error(`${label}.cursorSurfaceIdentity must match symbol availability`);
  }
  if (
    cursorSurfaceIdentityRequired && cursorSurfaceIdentity !== null &&
    item.symbolLocation === undefined
  ) {
    throw new Error(`${label}.cursorSurfaceIdentity requires symbolLocation`);
  }
  if (
    cursorSurfaceIdentity && item.symbolLocation !== undefined &&
    !isDeepStrictEqual(cursorSurfaceIdentity.location, item.symbolLocation)
  ) {
    throw new Error(`${label}.cursorSurfaceIdentity must match symbolLocation`);
  }
  return {
    ...(authoringContext === undefined
      ? {}
      : { authoringContext }),
    caseId,
    ...(cursorSurfaceIdentity === undefined
      ? {}
      : { cursorSurfaceIdentity }),
    decision,
    definitions,
    diagnostics,
    ...(formulaDecision === undefined ? {} : { formulaDecision }),
    ...(item.interpretations === undefined
      ? {}
      : {
          interpretations: item.interpretations as NonNullable<
            AuthoredScientificObservation["interpretations"]
          >,
        }),
    prepareRename,
    proofGrounded: item.proofGrounded,
    references,
    relations,
    renameEdits,
    ...(item.renameSafety === undefined
      ? {}
      : {
          renameSafety: nonemptyString(
            item.renameSafety,
            `${label}.renameSafety`,
          ),
        }),
    symbol,
    ...(item.symbolLocation === undefined
      ? {}
      : {
          symbolLocation: location(
            item.symbolLocation,
            `${label}.symbolLocation`,
          ),
        }),
    ...(item.surfaceAuthorizations === undefined
      ? {}
      : {
          surfaceAuthorizations: parseSurfaceAuthorizations(
            item.surfaceAuthorizations,
            `${label}.surfaceAuthorizations`,
          ),
        }),
  };
}

function parseCursorSurfaceIdentity(
  value: unknown,
  label: string,
): NonNullable<AuthoredScientificObservation["cursorSurfaceIdentity"]> | null {
  if (value === null) return null;
  const item = record(value, label);
  exact(item, ["entityId", "location", "occurrenceId"], label);
  const occurrenceId = parseSourceOccurrenceId(
    item.occurrenceId,
    `${label}.occurrenceId`,
  );
  const identity = {
    entityId: item.entityId === null
      ? null
      : parseEntityId(item.entityId, `${label}.entityId`),
    location: location(item.location, `${label}.location`),
    occurrenceId,
  };
  if (identity.location.fileId !== occurrenceId.fileId) {
    throw new Error(`${label}.occurrenceId must match location.fileId`);
  }
  return identity;
}

function parseSurfaceAuthorizations(
  value: unknown,
  label: string,
): NonNullable<AuthoredScientificObservation["surfaceAuthorizations"]> {
  const item = record(value, label);
  exact(
    item,
    ["definition", "prepareRename", "references", "rename"],
    label,
  );
  return {
    definition: parseSurfaceAuthorization(item.definition, `${label}.definition`),
    prepareRename: parseSurfaceAuthorization(
      item.prepareRename,
      `${label}.prepareRename`,
    ),
    references: parseSurfaceAuthorization(item.references, `${label}.references`),
    rename: parseSurfaceAuthorization(item.rename, `${label}.rename`),
  };
}

function parseSurfaceAuthorization(
  value: unknown,
  label: string,
): NonNullable<
  AuthoredScientificObservation["surfaceAuthorizations"]
>["definition"] {
  const item = record(value, label);
  const status = checked(
    item.status,
    /^(?:authorized|refused)$/u,
    `${label}.status`,
  ) as "authorized" | "refused";
  if (status === "authorized") {
    exact(item, ["entityId", "focusOccurrenceId", "status"], label);
    return {
      entityId: parseEntityId(item.entityId, `${label}.entityId`),
      focusOccurrenceId: parseSourceOccurrenceId(
        item.focusOccurrenceId,
        `${label}.focusOccurrenceId`,
      ),
      status,
    };
  }
  exact(item, ["refusalKind", "status"], label);
  return {
    refusalKind: checked(
      item.refusalKind,
      /^(?:unsupported|ambiguous|conflicting|engine-limit|incomplete-source|non-editable|invalid-replacement|capture)$/u,
      `${label}.refusalKind`,
    ) as Extract<
      NonNullable<
        AuthoredScientificObservation["surfaceAuthorizations"]
      >["definition"],
      { readonly status: "refused" }
    >["refusalKind"],
    status,
  };
}

function parseEntityId(
  value: unknown,
  label: string,
): Extract<
  NonNullable<
    AuthoredScientificObservation["surfaceAuthorizations"]
  >["definition"],
  { readonly status: "authorized" }
>["entityId"] {
  const item = record(value, label);
  exact(item, ["anchor", "componentId", "kind", "scopePath"], label);
  return {
    anchor: parseSourceOccurrenceId(item.anchor, `${label}.anchor`),
    componentId: nonemptyString(item.componentId, `${label}.componentId`),
    kind: nonemptyString(item.kind, `${label}.kind`),
    scopePath: array(item.scopePath, `${label}.scopePath`).map((value, index) =>
      integer(value, `${label}.scopePath[${index}]`)
    ),
  };
}

function parseSourceOccurrenceId(
  value: unknown,
  label: string,
): Extract<
  NonNullable<
    AuthoredScientificObservation["surfaceAuthorizations"]
  >["definition"],
  { readonly status: "authorized" }
>["focusOccurrenceId"] {
  const item = record(value, label);
  exact(item, ["documentVersion", "fileId", "localId"], label);
  return {
    documentVersion: integer(item.documentVersion, `${label}.documentVersion`),
    fileId: nonemptyString(item.fileId, `${label}.fileId`),
    localId: integer(item.localId, `${label}.localId`),
  };
}

function locations(value: unknown, label: string) {
  return array(value, label).map((item, index) =>
    location(item, `${label}[${index}]`),
  );
}
function parseObservedFormulaDecision(
  value: unknown,
  label: string,
): NonNullable<AuthoredScientificObservation["formulaDecision"]> {
  const item = record(value, label);
  exact(item, ["location", "status"], label);
  return {
    location: item.location === null
      ? null
      : location(item.location, `${label}.location`),
    status: checked(
      item.status,
      /^(?:ambiguous|conflicting|conventional|engine-limited|established|partial|unsupported)$/u,
      `${label}.status`,
    ) as NonNullable<
      AuthoredScientificObservation["formulaDecision"]
    >["status"],
  };
}
function location(value: unknown, label: string) {
  const item = record(value, label);
  exact(item, ["fileId", "path", "range"], label);
  return {
    fileId: nonemptyString(item.fileId, `${label}.fileId`),
    path: nonemptyString(item.path, `${label}.path`),
    range: sourceRange(item.range, `${label}.range`),
  };
}
function sourceRange(value: unknown, label: string) {
  const item = record(value, label);
  exact(item, ["endOffset", "startOffset"], label);
  const startOffset = integer(item.startOffset, `${label}.startOffset`);
  const endOffset = integer(item.endOffset, `${label}.endOffset`);
  if (startOffset > endOffset) throw new Error(`${label} is reversed`);
  return { endOffset, startOffset };
}
function array(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value;
}
function string(value: unknown, label: string): string {
  if (typeof value !== "string") throw new Error(`${label} must be a string`);
  return value;
}
function nonemptyString(value: unknown, label: string): string {
  const parsed = string(value, label);
  if (!parsed.trim()) throw new Error(`${label} must be non-empty`);
  return parsed;
}

function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return value;
}
function requiredPath(name: string): string {
  const value = required(name);
  return isAbsolute(value) ? value : resolve(process.cwd(), value);
}
function output(command: string, args: readonly string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0)
    throw new Error(result.stderr || `${command} failed`);
  return result.stdout.trim();
}
function run(
  command: string,
  args: readonly string[],
  environment: Readonly<Record<string, string>> = {},
): void {
  const result = spawnSync(command, args, {
    env: { ...process.env, ...environment },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")} failed`);
}
function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`${label} must be an object`);
  return value as Record<string, unknown>;
}
function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  label: string,
): void {
  if (
    JSON.stringify(Object.keys(value).sort()) !==
    JSON.stringify([...keys].sort())
  )
    throw new Error(`${label} has unexpected or missing fields`);
}
function strings(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string"))
    throw new Error(`${label} must be a string array`);
  return value;
}
function parseMathAuthoringFindings(
  value: unknown,
  label: string,
): readonly MathAuthoringContextFailure[] {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value.map((raw, index) => {
    const itemLabel = `${label}[${index}]`;
    const item = record(raw, itemLabel);
    const allowed = new Set(["actual", "expected", "kind", "path"]);
    if (
      !("kind" in item) || !("path" in item) ||
      Object.keys(item).some((key) => !allowed.has(key))
    ) {
      throw new Error(`${itemLabel} has unexpected or missing fields`);
    }
    const kind = mathAuthoringFailureKind(item.kind, `${itemLabel}.kind`);
    if (typeof item.path !== "string" || item.path.trim().length === 0)
      throw new Error(`${itemLabel}.path must be a nonempty string`);
    return {
      ...(Object.hasOwn(item, "actual") ? { actual: item.actual } : {}),
      ...(Object.hasOwn(item, "expected") ? { expected: item.expected } : {}),
      kind,
      path: item.path,
    };
  });
}
function mathAuthoringFailureKind(
  value: unknown,
  label: string,
): MathAuthoringFailureKind {
  const kinds = [
    "authority-escalation",
    "false-conflict",
    "mismatch",
    "missing",
    "unexpected",
    "unsafe-lifecycle",
    "wrong-anchor",
  ] as const satisfies readonly MathAuthoringFailureKind[];
  const kind = kinds.find((candidate) => candidate === value);
  if (!kind) throw new Error(`${label} is invalid`);
  return kind;
}
function integer(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0)
    throw new Error(`${label} must be a nonnegative integer`);
  return value;
}
function checked(value: unknown, pattern: RegExp, label: string): string {
  if (typeof value !== "string" || !pattern.test(value))
    throw new Error(`${label} is invalid`);
  return value;
}
