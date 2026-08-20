import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { parseFreshBlindReservation } from "./check-fresh-blind-reservation";
import { sha256 } from "./fresh-blind-evidence";
import { parseFreshBlindReceipt } from "./fresh-blind-receipt";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

export interface FreshBlindPublishedResult {
  readonly artifactDigest: string;
  readonly artifactId: string;
  readonly artifactUrl: string;
  readonly candidateSha: string;
  readonly releaseId: string;
  readonly reservationSha256: string;
  readonly runUrl: string;
  readonly status: "completed" | "safety-failed" | "execution-error";
  readonly terminalReceiptSha256: string | null;
}

export function freshBlindResultMarker(
  result: FreshBlindPublishedResult,
): string {
  validatePublishedResult(result);
  return `<!-- semath-fresh-blind-result:${result.releaseId}:${result.candidateSha}:${result.status}:${result.terminalReceiptSha256 ?? "none"} -->`;
}

export function buildFreshBlindResultComment(
  result: FreshBlindPublishedResult,
): string {
  const marker = freshBlindResultMarker(result);
  const terminal = result.terminalReceiptSha256
    ? `Terminal receipt SHA-256: \`${result.terminalReceiptSha256}\`.`
    : "No terminal receipt was produced; the reservation remains spent and publication is blocked.";
  return [
    marker,
    `Fresh-blind release \`${result.releaseId}\` finished with status **${result.status}** at candidate \`${result.candidateSha}\`.`,
    terminal,
    `Reservation SHA-256: \`${result.reservationSha256}\`. Artifact ${result.artifactId} digest: \`${result.artifactDigest}\`.`,
    `[Workflow run](${result.runUrl}) · [immutable run artifact](${result.artifactUrl})`,
  ].join("\n\n");
}

if (import.meta.main) await publish();

async function publish(): Promise<void> {
  assertFreshBlindLinuxX64();
  const reservationBytes = await readFile(
    requiredPath("SEMATH_FRESH_BLIND_RESERVATION"),
  );
  const reservation = parseFreshBlindReservation(
    JSON.parse(reservationBytes.toString("utf8")) as unknown,
  );
  assertFreshBlindWorkflowBoundary(
    freshBlindWorkflowBoundaryFromEnvironment(reservation.candidateSha),
  );
  const terminalPath = requiredPath("SEMATH_FRESH_BLIND_RECEIPT");
  let terminalReceiptSha256: string | null = null;
  let status: FreshBlindPublishedResult["status"] = "execution-error";
  if (existsSync(terminalPath)) {
    const terminalBytes = await readFile(terminalPath);
    const receipt = parseFreshBlindReceipt(
      JSON.parse(terminalBytes.toString("utf8")) as unknown,
    );
    if (receipt.status === "started")
      throw new Error("result publisher requires a terminal receipt");
    if (
      receipt.release.fixtureId !== reservation.releaseId ||
      receipt.release.fixtureSeal !== reservation.fixtureSeal ||
      receipt.provenance.candidateCommit !== reservation.candidateSha
    )
      throw new Error("terminal receipt does not match the reservation");
    if (
      receipt.reservation.sha256 !== sha256(reservationBytes) ||
      receipt.reservation.ledgerMarker !== reservation.marker
    )
      throw new Error("terminal receipt reservation digest does not match");
    terminalReceiptSha256 = sha256(terminalBytes);
    status = receipt.status;
  }
  const repository = required("GITHUB_REPOSITORY");
  const runId = required("GITHUB_RUN_ID");
  const artifactId = required("SEMATH_RELEASE_ARTIFACT_ID");
  const result: FreshBlindPublishedResult = {
    artifactDigest: normalizeArtifactDigest(
      required("SEMATH_RELEASE_ARTIFACT_DIGEST"),
    ),
    artifactId,
    artifactUrl: `https://github.com/${repository}/actions/runs/${runId}/artifacts/${artifactId}`,
    candidateSha: reservation.candidateSha,
    releaseId: reservation.releaseId,
    reservationSha256: sha256(reservationBytes),
    runUrl: `https://github.com/${repository}/actions/runs/${runId}`,
    status,
    terminalReceiptSha256,
  };
  const response = await fetch(
    `https://api.github.com/repos/${repository}/issues/${required("SEMATH_RELEASE_LEDGER_ISSUE")}/comments`,
    {
      body: JSON.stringify({ body: buildFreshBlindResultComment(result) }),
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${required("GITHUB_TOKEN")}`,
        "X-GitHub-Api-Version": "2022-11-28",
      },
      method: "POST",
    },
  );
  if (!response.ok)
    throw new Error(
      `failed to publish fresh blind terminal result: GitHub ${response.status}`,
    );
  console.log(`fresh blind result published: ${result.status}`);
}

function normalizeArtifactDigest(value: string): string {
  const normalized = value.startsWith("sha256:") ? value.slice(7) : value;
  if (!/^[0-9a-f]{64}$/u.test(normalized))
    throw new Error("artifact digest must be SHA-256");
  return normalized;
}

function validatePublishedResult(result: FreshBlindPublishedResult): void {
  if (!/^v0\.[1-9][0-9]*$/u.test(result.releaseId))
    throw new Error("published result release id is invalid");
  if (!/^[0-9a-f]{40}$/u.test(result.candidateSha))
    throw new Error("published result candidate SHA is invalid");
  if (
    !/^[0-9a-f]{64}$/u.test(result.reservationSha256) ||
    !/^[0-9a-f]{64}$/u.test(result.artifactDigest)
  )
    throw new Error("published result digest is invalid");
  if (
    result.terminalReceiptSha256 !== null &&
    !/^[0-9a-f]{64}$/u.test(result.terminalReceiptSha256)
  )
    throw new Error("published terminal receipt digest is invalid");
  if (!/^[1-9][0-9]*$/u.test(result.artifactId))
    throw new Error("published artifact id is invalid");
  for (const [label, value] of [
    ["run URL", result.runUrl],
    ["artifact URL", result.artifactUrl],
  ] as const) {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.hostname !== "github.com")
      throw new Error(`published ${label} is invalid`);
  }
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
