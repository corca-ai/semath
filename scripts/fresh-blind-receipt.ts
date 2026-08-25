import { mkdir, open, readFile, writeFile } from "node:fs/promises";
import { dirname, extname } from "node:path";
import { sha256 } from "./fresh-blind-evidence";
import type { FreshBlindPreflightManifest } from "./fresh-blind-preflight-manifest";
import type { FreshBlindReservation } from "./check-fresh-blind-reservation";

export type FreshBlindTerminalStatus =
  "completed" | "safety-failed" | "execution-error";

export interface FreshBlindReceiptArtifacts {
  readonly checksumManifestSha256: string;
  readonly committedWasmSha256: string;
  readonly evaluationSha256: string | null;
  readonly lifecycleSha256: string | null;
  readonly nativeSha256: string;
  readonly npmTarballSha256: string;
  readonly preflightManifestSha256: string;
  readonly rebuiltWasmSha256: string;
}

interface FreshBlindReceiptIdentity {
  readonly artifacts: FreshBlindReceiptArtifacts;
  readonly contracts: Omit<
    FreshBlindPreflightManifest["contracts"],
    "receiptPolicyVersion"
  > & { readonly receiptPolicyVersion: 2 | 3 };
  readonly provenance: FreshBlindPreflightManifest["provenance"] & {
    readonly runAttempt: string;
    readonly runId: string;
  };
  readonly receiptPolicyVersion: 2 | 3;
  readonly release: FreshBlindPreflightManifest["release"];
  readonly reservation: {
    readonly ledgerMarker: string;
    readonly sha256: string;
  };
  readonly schemaVersion: 2 | 3;
  readonly startedAt: string;
}

export interface FreshBlindStartedReceipt extends FreshBlindReceiptIdentity {
  readonly status: "started";
}

export interface FreshBlindTerminalReceipt extends FreshBlindReceiptIdentity {
  readonly completedAt: string;
  readonly result: unknown;
  readonly status: FreshBlindTerminalStatus;
}

export type FreshBlindReleaseReceipt =
  FreshBlindStartedReceipt | FreshBlindTerminalReceipt;

export function createFreshBlindStartedReceipt(input: {
  readonly manifest: FreshBlindPreflightManifest;
  readonly manifestSha256: string;
  readonly reservation: FreshBlindReservation;
  readonly reservationSha256: string;
  readonly startedAt: string;
}): FreshBlindStartedReceipt {
  const { manifest, reservation } = input;
  if (reservation.candidateSha !== manifest.provenance.candidateCommit)
    throw new Error("reservation candidate does not match pre-blind manifest");
  if (
    reservation.fixtureSeal !== manifest.release.fixtureSeal ||
    reservation.releaseId !== manifest.release.fixtureId
  )
    throw new Error("reservation release does not match pre-blind manifest");
  const receipt = parseFreshBlindReceipt({
    artifacts: {
      checksumManifestSha256: manifest.artifacts.checksumManifestSha256,
      committedWasmSha256: manifest.artifacts.committedWasmSha256,
      evaluationSha256: null,
      lifecycleSha256: null,
      nativeSha256: manifest.artifacts.nativeSha256,
      npmTarballSha256: manifest.artifacts.npmTarballSha256,
      preflightManifestSha256: input.manifestSha256,
      rebuiltWasmSha256: manifest.artifacts.rebuiltWasmSha256,
    },
    contracts: manifest.contracts,
    provenance: {
      ...manifest.provenance,
      runAttempt: reservation.runAttempt,
      runId: reservation.runId,
    },
    receiptPolicyVersion: 3,
    release: manifest.release,
    reservation: {
      ledgerMarker: reservation.marker,
      sha256: input.reservationSha256,
    },
    schemaVersion: 3,
    startedAt: input.startedAt,
    status: "started",
  });
  if (receipt.status !== "started")
    throw new Error("created receipt is not started");
  return receipt;
}

export function createFreshBlindExecutionErrorReceipt(
  started: FreshBlindStartedReceipt,
  error: string,
): FreshBlindTerminalReceipt {
  const terminal = parseFreshBlindReceipt({
    ...started,
    completedAt: new Date().toISOString(),
    result: {
      error,
      evaluation: null,
    },
    status: "execution-error",
  });
  if (terminal.status === "started") {
    throw new Error("execution error receipt must be terminal");
  }
  return terminal;
}

/** The started and terminal records are immutable separate files. Every
 * successful write also produces a content-addressed copy for durable records. */
export async function reserveFreshBlindReceipt(
  path: string,
  receipt: FreshBlindStartedReceipt,
): Promise<void> {
  const parsed = parseFreshBlindReceipt(receipt);
  if (parsed.status !== "started")
    throw new Error("receipt reservation requires a started receipt");
  await writeExclusiveReceipt(freshBlindStartedReceiptPath(path), parsed);
}

export async function finalizeFreshBlindReceipt(
  path: string,
  receipt: FreshBlindTerminalReceipt,
): Promise<void> {
  const startedPath = freshBlindStartedReceiptPath(path);
  const current = parseFreshBlindReceipt(
    JSON.parse(await readFile(startedPath, "utf8")) as unknown,
  );
  const planned = planFreshBlindReceiptTransition(current, receipt);
  await writeExclusiveReceipt(path, planned);
}

export function freshBlindStartedReceiptPath(path: string): string {
  const extension = extname(path);
  return extension
    ? `${path.slice(0, -extension.length)}.started${extension}`
    : `${path}.started`;
}

export function freshBlindContentAddressedPath(
  path: string,
  digest: string,
): string {
  if (!/^[0-9a-f]{64}$/u.test(digest))
    throw new Error("content-addressed receipt path requires a SHA-256 digest");
  const extension = extname(path);
  return extension
    ? `${path.slice(0, -extension.length)}.${digest}${extension}`
    : `${path}.${digest}`;
}

export function planFreshBlindReceiptTransition(
  current: FreshBlindReleaseReceipt,
  terminal: FreshBlindReleaseReceipt,
): FreshBlindTerminalReceipt {
  const parsedCurrent = parseFreshBlindReceipt(current);
  const parsedTerminal = parseFreshBlindReceipt(terminal);
  if (parsedCurrent.status !== "started")
    throw new Error(
      "a terminal transition requires an existing started receipt",
    );
  if (parsedTerminal.status === "started")
    throw new Error(
      "a final receipt requires a terminal status and completedAt",
    );
  if (!sameReservedExecution(parsedCurrent, parsedTerminal))
    throw new Error(
      "a terminal receipt must describe the same reserved execution",
    );
  return parsedTerminal;
}

export function parseFreshBlindReceipt(
  value: unknown,
): FreshBlindReleaseReceipt {
  const root = record(value, "fresh blind receipt");
  const status = string(root.status, "fresh blind receipt.status");
  const terminal = status !== "started";
  if (!terminal && status !== "started")
    throw new Error("fresh blind receipt.status is invalid");
  if (
    terminal &&
    status !== "completed" &&
    status !== "safety-failed" &&
    status !== "execution-error"
  )
    throw new Error("fresh blind receipt.status is invalid");
  exact(
    root,
    terminal
      ? [
          "artifacts",
          "completedAt",
          "contracts",
          "provenance",
          "receiptPolicyVersion",
          "release",
          "reservation",
          "result",
          "schemaVersion",
          "startedAt",
          "status",
        ]
      : [
          "artifacts",
          "contracts",
          "provenance",
          "receiptPolicyVersion",
          "release",
          "reservation",
          "schemaVersion",
          "startedAt",
          "status",
        ],
    "fresh blind receipt",
  );
  const schemaVersion = root.schemaVersion;
  const receiptPolicyVersion = root.receiptPolicyVersion;
  if (schemaVersion !== 2 && schemaVersion !== 3) {
    throw new Error(
      "fresh blind receipt schemaVersion and receiptPolicyVersion must both be 2 or both be 3",
    );
  }
  if (receiptPolicyVersion !== 2 && receiptPolicyVersion !== 3) {
    throw new Error(
      "fresh blind receipt schemaVersion and receiptPolicyVersion must both be 2 or both be 3",
    );
  }
  if (schemaVersion !== receiptPolicyVersion) {
    throw new Error(
      "fresh blind receipt schemaVersion and receiptPolicyVersion must both be 2 or both be 3",
    );
  }
  const startedAt = iso(root.startedAt, "fresh blind receipt.startedAt");

  const release = parseRelease(root.release);
  const contracts = parseContracts(root.contracts, receiptPolicyVersion);
  const provenance = parseProvenance(root.provenance);
  const reservation = parseReservation(root.reservation);
  const artifacts = parseArtifacts(root.artifacts);
  const expectedMarker = `<!-- semath-fresh-blind-reservation:${release.fixtureId}:${release.fixtureSeal}:${provenance.candidateCommit} -->`;
  if (reservation.ledgerMarker !== expectedMarker)
    throw new Error(
      "fresh blind receipt reservation marker does not match execution identity",
    );
  const identity: FreshBlindReceiptIdentity = {
    artifacts,
    contracts,
    provenance,
    receiptPolicyVersion,
    release,
    reservation,
    schemaVersion,
    startedAt,
  };
  if (status === "started") {
    if (
      artifacts.evaluationSha256 !== null ||
      artifacts.lifecycleSha256 !== null
    )
      throw new Error("started receipt cannot claim terminal evidence");
    return { ...identity, status };
  }
  const completedAt = iso(root.completedAt, "fresh blind receipt.completedAt");
  if (
    (status === "completed" || status === "safety-failed") &&
    (!artifacts.evaluationSha256 || !artifacts.lifecycleSha256)
  )
    throw new Error(
      "evaluated terminal receipt requires evaluation and lifecycle digests",
    );
  const result = parseTerminalResult(
    root.result,
    status,
    receiptPolicyVersion,
  );
  if (status !== "execution-error") {
    const lifecycle = record(
      record(result, "fresh blind receipt.result").lifecycle,
      "fresh blind receipt.result.lifecycle",
    );
    if (
      lifecycle.fixtureId !== release.fixtureId ||
      lifecycle.fixtureSeal !== release.fixtureSeal
    )
      throw new Error(
        "fresh blind receipt lifecycle does not match release identity",
      );
  }
  return { ...identity, completedAt, result, status };
}

function parseTerminalResult(
  value: unknown,
  status: FreshBlindTerminalStatus,
  receiptPolicyVersion: 2 | 3,
): unknown {
  const result = record(value, "fresh blind receipt.result");
  if (status === "execution-error") {
    exact(result, ["error", "evaluation"], "fresh blind receipt.result");
    nonempty(result.error, "fresh blind receipt.result.error");
    if (result.evaluation !== null)
      record(result.evaluation, "fresh blind receipt.result.evaluation");
    return value;
  }
  if (receiptPolicyVersion === 3 && !("authoringSafety" in result)) {
    throw new Error(
      "fresh blind receipt.result.authoringSafety is required by policy 3",
    );
  }
  exact(
    result,
    receiptPolicyVersion === 3
      ? [
          "authoringSafety",
          "evaluation",
          "facetFailureIds",
          "lifecycle",
          "safety",
          "validation",
        ]
      : [
          "evaluation",
          "facetFailureIds",
          "lifecycle",
          "safety",
          "validation",
        ],
    "fresh blind receipt.result",
  );
  const authoringSafety = receiptPolicyVersion === 3
    ? parseAuthoringSafety(result.authoringSafety)
    : { cases: 0, failures: 0 };
  record(result.evaluation, "fresh blind receipt.result.evaluation");
  const facetFailures = uniqueNonemptyStrings(
    result.facetFailureIds,
    "fresh blind receipt.result.facetFailureIds",
  );
  const lifecycle = record(
    result.lifecycle,
    "fresh blind receipt.result.lifecycle",
  );
  exact(
    lifecycle,
    [
      "comparedProbes",
      "comparedStages",
      "fixtureId",
      "fixtureSeal",
      "schemaVersion",
    ],
    "fresh blind receipt.result.lifecycle",
  );
  nonnegative(
    lifecycle.comparedProbes,
    "fresh blind receipt.result.lifecycle.comparedProbes",
  );
  nonnegative(
    lifecycle.comparedStages,
    "fresh blind receipt.result.lifecycle.comparedStages",
  );
  checked(
    lifecycle.fixtureId,
    /^v0\.[1-9][0-9]*$/u,
    "fresh blind receipt.result.lifecycle.fixtureId",
  );
  digest(
    lifecycle.fixtureSeal,
    "fresh blind receipt.result.lifecycle.fixtureSeal",
  );
  literal(
    lifecycle.schemaVersion,
    1,
    "fresh blind receipt.result.lifecycle.schemaVersion",
  );
  const safety = record(result.safety, "fresh blind receipt.result.safety");
  exact(
    safety,
    [
      "diagnosticsOverLimit",
      "diagnosticsOverLimitIds",
      "falseConflict",
      "falseConflictIds",
      "falseEstablishment",
      "falseEstablishmentIds",
      "unsafeNavigationOrEditCaseIds",
      "unsafeNavigationOrEditLocations",
    ],
    "fresh blind receipt.result.safety",
  );
  const unsafeCounts = [
    nonnegative(
      safety.diagnosticsOverLimit,
      "fresh blind receipt.result.safety.diagnosticsOverLimit",
    ),
    nonnegative(
      safety.falseConflict,
      "fresh blind receipt.result.safety.falseConflict",
    ),
    nonnegative(
      safety.falseEstablishment,
      "fresh blind receipt.result.safety.falseEstablishment",
    ),
    nonnegative(
      safety.unsafeNavigationOrEditLocations,
      "fresh blind receipt.result.safety.unsafeNavigationOrEditLocations",
    ),
  ] as const;
  const diagnosticsOverLimitIds = uniqueNonemptyStrings(
    safety.diagnosticsOverLimitIds,
    "fresh blind receipt.result.safety.diagnosticsOverLimitIds",
  );
  const falseConflictIds = uniqueNonemptyStrings(
    safety.falseConflictIds,
    "fresh blind receipt.result.safety.falseConflictIds",
  );
  const falseEstablishmentIds = uniqueNonemptyStrings(
    safety.falseEstablishmentIds,
    "fresh blind receipt.result.safety.falseEstablishmentIds",
  );
  const unsafeNavigationOrEditCaseIds = uniqueNonemptyStrings(
    safety.unsafeNavigationOrEditCaseIds,
    "fresh blind receipt.result.safety.unsafeNavigationOrEditCaseIds",
  );
  if (
    diagnosticsOverLimitIds.length !== unsafeCounts[0] ||
    falseConflictIds.length !== unsafeCounts[1] ||
    falseEstablishmentIds.length !== unsafeCounts[2] ||
    unsafeNavigationOrEditCaseIds.length > unsafeCounts[3]
  ) {
    throw new Error(
      "fresh blind receipt safety counts disagree with failure ids",
    );
  }
  parseValidation(result.validation, receiptPolicyVersion);
  const unsafe =
    authoringSafety.failures > 0 || facetFailures.length > 0 ||
    unsafeCounts.some((count) => count > 0);
  if (status === "completed" && unsafe)
    throw new Error("completed receipt cannot contain safety failures");
  if (status === "safety-failed" && !unsafe)
    throw new Error("safety-failed receipt must identify a safety failure");
  return value;
}

function parseAuthoringSafety(
  value: unknown,
): { readonly cases: number; readonly failures: number } {
  const summary = record(
    value,
    "fresh blind receipt.result.authoringSafety",
  );
  exact(
    summary,
    ["cases", "failures"],
    "fresh blind receipt.result.authoringSafety",
  );
  const cases = nonnegative(
    summary.cases,
    "fresh blind receipt.result.authoringSafety.cases",
  );
  if (cases === 0) {
    throw new Error(
      "fresh blind receipt.result.authoringSafety.cases must be positive",
    );
  }
  if (!Array.isArray(summary.failures)) {
    throw new Error(
      "fresh blind receipt.result.authoringSafety.failures must be an array",
    );
  }
  const kinds = new Set([
    "authority-escalation",
    "false-conflict",
    "mismatch",
    "missing",
    "unexpected",
    "unsafe-lifecycle",
    "wrong-anchor",
  ]);
  for (const [index, value] of summary.failures.entries()) {
    const path = `fresh blind receipt.result.authoringSafety.failures[${index}]`;
    const failure = record(value, path);
    const keys = Object.keys(failure);
    if (
      !keys.includes("kind") || !keys.includes("path") ||
      keys.some((key) => !["actual", "expected", "kind", "path"].includes(key))
    ) {
      throw new Error(`${path} has unexpected or missing fields`);
    }
    if (typeof failure.kind !== "string" || !kinds.has(failure.kind)) {
      throw new Error(`${path}.kind is invalid`);
    }
    nonempty(failure.path, `${path}.path`);
  }
  return { cases, failures: summary.failures.length };
}

function parseValidation(value: unknown, receiptPolicyVersion: 2 | 3): void {
  const validation = record(value, "fresh blind receipt.result.validation");
  const currentKeys = ["entityDecisions", "fields", "formulaDecisions"];
  const decisionDomainKeys = receiptPolicyVersion === 3
    ? currentKeys
    : currentKeys.filter((key) => key in validation);
  exact(
    validation,
    [
      "decisions",
      ...decisionDomainKeys,
      "families",
      "laws",
      "maximumMathSimilarity",
      "maximumProseSimilarity",
      "probes",
      "scenarios",
    ],
    "fresh blind receipt.result.validation",
  );
  for (const [label, value] of [
    ["decisions", validation.decisions],
    ...decisionDomainKeys.map((key) => [key, validation[key]] as const),
    ["families", validation.families],
  ] as const) {
    const counts = record(
      value,
      `fresh blind receipt.result.validation.${label}`,
    );
    if (!Object.keys(counts).length) {
      throw new Error(
        `fresh blind receipt.result.validation.${label} must not be empty`,
      );
    }
    for (const [key, count] of Object.entries(counts)) {
      if (!key.trim()) {
        throw new Error(
          `fresh blind receipt.result.validation.${label} has an empty key`,
        );
      }
      nonnegative(
        count,
        `fresh blind receipt.result.validation.${label}.${key}`,
      );
    }
  }
  for (const key of ["laws", "probes", "scenarios"] as const) {
    nonnegative(
      validation[key],
      `fresh blind receipt.result.validation.${key}`,
    );
  }
  for (const key of [
    "maximumMathSimilarity",
    "maximumProseSimilarity",
  ] as const) {
    const similarity = validation[key];
    if (
      typeof similarity !== "number" ||
      !Number.isFinite(similarity) ||
      similarity < 0 ||
      similarity > 1
    ) {
      throw new Error(
        `fresh blind receipt.result.validation.${key} must be between zero and one`,
      );
    }
  }
}

function sameReservedExecution(
  current: FreshBlindStartedReceipt,
  terminal: FreshBlindTerminalReceipt,
): boolean {
  const stripTerminalArtifacts = (
    artifacts: FreshBlindReceiptArtifacts,
  ): FreshBlindReceiptArtifacts => ({
    ...artifacts,
    evaluationSha256: null,
    lifecycleSha256: null,
  });
  const identity = (receipt: FreshBlindReleaseReceipt) => ({
    artifacts: stripTerminalArtifacts(receipt.artifacts),
    contracts: receipt.contracts,
    provenance: receipt.provenance,
    receiptPolicyVersion: receipt.receiptPolicyVersion,
    release: receipt.release,
    reservation: receipt.reservation,
    schemaVersion: receipt.schemaVersion,
    startedAt: receipt.startedAt,
  });
  return (
    JSON.stringify(identity(current)) === JSON.stringify(identity(terminal))
  );
}

async function writeExclusiveReceipt(
  path: string,
  receipt: FreshBlindReleaseReceipt,
): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const bytes = `${JSON.stringify(receipt, null, 2)}\n`;
  const digest = sha256(bytes);
  const addressed = await open(
    freshBlindContentAddressedPath(path, digest),
    "wx",
  );
  try {
    await addressed.writeFile(bytes);
    await addressed.sync();
  } finally {
    await addressed.close();
  }
  await writeFile(`${path}.sha256`, `${digest}  ${path.split("/").at(-1)}\n`, {
    flag: "wx",
  });
  // Publish the canonical record last. Its presence means both durable
  // content-addressed evidence and its checksum sidecar already exist.
  const file = await open(path, "wx");
  try {
    await file.writeFile(bytes);
    await file.sync();
  } finally {
    await file.close();
  }
}

function parseRelease(value: unknown): FreshBlindPreflightManifest["release"] {
  const item = record(value, "fresh blind receipt.release");
  exact(
    item,
    ["fixtureId", "fixtureSeal", "fixtureSha256", "packageVersion"],
    "fresh blind receipt.release",
  );
  const fixtureId = checked(
    item.fixtureId,
    /^v0\.[1-9][0-9]*$/u,
    "fresh blind receipt.release.fixtureId",
  );
  const fixtureSeal = digest(
    item.fixtureSeal,
    "fresh blind receipt.release.fixtureSeal",
  );
  const fixtureSha256 = digest(
    item.fixtureSha256,
    "fresh blind receipt.release.fixtureSha256",
  );
  literal(
    item.packageVersion,
    "0.18.0",
    "fresh blind receipt.release.packageVersion",
  );
  return { fixtureId, fixtureSeal, fixtureSha256, packageVersion: "0.18.0" };
}

function parseContracts(
  value: unknown,
  receiptPolicyVersion: 2 | 3,
): FreshBlindReceiptIdentity["contracts"] {
  const item = record(value, "fresh blind receipt.contracts");
  exact(
    item,
    [
      "packSchemaVersion",
      "protocolVersion",
      "receiptPolicyVersion",
      "wasmtexSyntaxSchemaVersion",
    ],
    "fresh blind receipt.contracts",
  );
  literal(
    item.packSchemaVersion,
    12,
    "fresh blind receipt.contracts.packSchemaVersion",
  );
  literal(
    item.protocolVersion,
    17,
    "fresh blind receipt.contracts.protocolVersion",
  );
  literal(
    item.receiptPolicyVersion,
    receiptPolicyVersion,
    "fresh blind receipt.contracts.receiptPolicyVersion",
  );
  literal(
    item.wasmtexSyntaxSchemaVersion,
    8,
    "fresh blind receipt.contracts.wasmtexSyntaxSchemaVersion",
  );
  return {
    packSchemaVersion: 12,
    protocolVersion: 17,
    receiptPolicyVersion,
    wasmtexSyntaxSchemaVersion: 8,
  };
}

function parseProvenance(
  value: unknown,
): FreshBlindStartedReceipt["provenance"] {
  const item = record(value, "fresh blind receipt.provenance");
  exact(
    item,
    [
      "builderIdentity",
      "candidateCommit",
      "candidateTree",
      "runAttempt",
      "runId",
      "runnerArch",
      "runnerImage",
      "runnerOs",
      "tools",
      "wasmtexCommit",
      "workflowFileSha256",
      "workflowRef",
      "workflowSha",
    ],
    "fresh blind receipt.provenance",
  );
  const tools = record(item.tools, "fresh blind receipt.provenance.tools");
  exact(
    tools,
    ["bun", "rust", "wasmBindgen"],
    "fresh blind receipt.provenance.tools",
  );
  literal(tools.bun, "1.3.14", "fresh blind receipt.provenance.tools.bun");
  literal(tools.rust, "1.96.0", "fresh blind receipt.provenance.tools.rust");
  literal(
    tools.wasmBindgen,
    "0.2.100",
    "fresh blind receipt.provenance.tools.wasmBindgen",
  );
  literal(item.runnerArch, "X64", "fresh blind receipt.provenance.runnerArch");
  literal(
    item.runnerImage,
    "ubuntu-24.04",
    "fresh blind receipt.provenance.runnerImage",
  );
  literal(item.runnerOs, "Linux", "fresh blind receipt.provenance.runnerOs");
  return {
    builderIdentity: nonempty(
      item.builderIdentity,
      "fresh blind receipt.provenance.builderIdentity",
    ),
    candidateCommit: commit(
      item.candidateCommit,
      "fresh blind receipt.provenance.candidateCommit",
    ),
    candidateTree: commit(
      item.candidateTree,
      "fresh blind receipt.provenance.candidateTree",
    ),
    runAttempt: checked(
      item.runAttempt,
      /^[1-9][0-9]*$/u,
      "fresh blind receipt.provenance.runAttempt",
    ),
    runId: checked(
      item.runId,
      /^[1-9][0-9]*$/u,
      "fresh blind receipt.provenance.runId",
    ),
    runnerArch: "X64",
    runnerImage: "ubuntu-24.04",
    runnerOs: "Linux",
    tools: { bun: "1.3.14", rust: "1.96.0", wasmBindgen: "0.2.100" },
    wasmtexCommit: commit(
      item.wasmtexCommit,
      "fresh blind receipt.provenance.wasmtexCommit",
    ),
    workflowFileSha256: digest(
      item.workflowFileSha256,
      "fresh blind receipt.provenance.workflowFileSha256",
    ),
    workflowRef: nonempty(
      item.workflowRef,
      "fresh blind receipt.provenance.workflowRef",
    ),
    workflowSha: commit(
      item.workflowSha,
      "fresh blind receipt.provenance.workflowSha",
    ),
  };
}

function parseReservation(
  value: unknown,
): FreshBlindStartedReceipt["reservation"] {
  const item = record(value, "fresh blind receipt.reservation");
  exact(item, ["ledgerMarker", "sha256"], "fresh blind receipt.reservation");
  return {
    ledgerMarker: checked(
      item.ledgerMarker,
      /^<!-- semath-fresh-blind-reservation:v0\.[1-9][0-9]*:[0-9a-f]{64}:[0-9a-f]{40} -->$/u,
      "fresh blind receipt.reservation.ledgerMarker",
    ),
    sha256: digest(item.sha256, "fresh blind receipt.reservation.sha256"),
  };
}

function parseArtifacts(value: unknown): FreshBlindReceiptArtifacts {
  const item = record(value, "fresh blind receipt.artifacts");
  exact(
    item,
    [
      "checksumManifestSha256",
      "committedWasmSha256",
      "evaluationSha256",
      "lifecycleSha256",
      "nativeSha256",
      "npmTarballSha256",
      "preflightManifestSha256",
      "rebuiltWasmSha256",
    ],
    "fresh blind receipt.artifacts",
  );
  const committedWasmSha256 = digest(
    item.committedWasmSha256,
    "fresh blind receipt.artifacts.committedWasmSha256",
  );
  const rebuiltWasmSha256 = digest(
    item.rebuiltWasmSha256,
    "fresh blind receipt.artifacts.rebuiltWasmSha256",
  );
  if (committedWasmSha256 !== rebuiltWasmSha256)
    throw new Error("fresh blind receipt committed and rebuilt WASM differ");
  return {
    checksumManifestSha256: digest(
      item.checksumManifestSha256,
      "fresh blind receipt.artifacts.checksumManifestSha256",
    ),
    committedWasmSha256,
    evaluationSha256: nullableDigest(
      item.evaluationSha256,
      "fresh blind receipt.artifacts.evaluationSha256",
    ),
    lifecycleSha256: nullableDigest(
      item.lifecycleSha256,
      "fresh blind receipt.artifacts.lifecycleSha256",
    ),
    nativeSha256: digest(
      item.nativeSha256,
      "fresh blind receipt.artifacts.nativeSha256",
    ),
    npmTarballSha256: digest(
      item.npmTarballSha256,
      "fresh blind receipt.artifacts.npmTarballSha256",
    ),
    preflightManifestSha256: digest(
      item.preflightManifestSha256,
      "fresh blind receipt.artifacts.preflightManifestSha256",
    ),
    rebuiltWasmSha256,
  };
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
function string(value: unknown, label: string): string {
  if (typeof value !== "string") throw new Error(`${label} must be a string`);
  return value;
}
function nonempty(value: unknown, label: string): string {
  const parsed = string(value, label);
  if (!parsed.trim()) throw new Error(`${label} must not be empty`);
  return parsed;
}
function checked(value: unknown, pattern: RegExp, label: string): string {
  const parsed = string(value, label);
  if (!pattern.test(parsed)) throw new Error(`${label} is invalid`);
  return parsed;
}
function digest(value: unknown, label: string): string {
  return checked(value, /^[0-9a-f]{64}$/u, label);
}
function nullableDigest(value: unknown, label: string): string | null {
  return value === null ? null : digest(value, label);
}
function commit(value: unknown, label: string): string {
  return checked(value, /^[0-9a-f]{40}$/u, label);
}
function literal<const T extends string | number>(
  value: unknown,
  expected: T,
  label: string,
): asserts value is T {
  if (value !== expected) throw new Error(`${label} must be ${expected}`);
}
function iso(value: unknown, label: string): string {
  return checked(
    value,
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u,
    label,
  );
}
function nonnegative(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0)
    throw new Error(`${label} must be a nonnegative integer`);
  return value;
}
function stringList(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string"))
    throw new Error(`${label} must be a string array`);
  return value;
}
function uniqueNonemptyStrings(
  value: unknown,
  label: string,
): readonly string[] {
  const parsed = stringList(value, label);
  if (
    parsed.some((item) => !item.trim()) ||
    new Set(parsed).size !== parsed.length
  ) {
    throw new Error(`${label} must contain unique non-empty strings`);
  }
  return parsed;
}
