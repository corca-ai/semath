import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { checkFreshBlindReservationIdentity } from "./check-fresh-blind-reservation";
import { sha256 } from "./fresh-blind-evidence";
import {
  createFreshBlindExecutionErrorReceipt,
  createFreshBlindStartedReceipt,
  finalizeFreshBlindReceipt,
  freshBlindStartedReceiptPath,
  parseFreshBlindReceipt,
  reserveFreshBlindReceipt,
  type FreshBlindStartedReceipt,
} from "./fresh-blind-receipt";
import { parseFreshBlindPreflightManifest } from "./fresh-blind-preflight-manifest";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

if (import.meta.main) await terminalize();

async function terminalize(): Promise<void> {
  assertFreshBlindLinuxX64();
  const candidateSha = required("SEMATH_CANDIDATE_SHA");
  assertFreshBlindWorkflowBoundary(
    freshBlindWorkflowBoundaryFromEnvironment(candidateSha),
  );
  const receiptPath = requiredPath("SEMATH_FRESH_BLIND_RECEIPT");
  if (existsSync(receiptPath)) {
    const receipt = parseFreshBlindReceipt(
      JSON.parse(await readFile(receiptPath, "utf8")) as unknown,
    );
    if (receipt.status === "started") {
      throw new Error("canonical fresh blind receipt cannot remain started");
    }
    console.log(
      `fresh blind terminal receipt already exists: ${receipt.status}`,
    );
    return;
  }

  const releaseId = required("SEMATH_RELEASE_ID");
  const fixturePath = requiredPath("SEMATH_FRESH_BLIND_FIXTURE");
  const reservationPath = requiredPath("SEMATH_FRESH_BLIND_RESERVATION");
  const manifestPath = requiredPath("SEMATH_FRESH_BLIND_PREFLIGHT_MANIFEST");
  const reservationBytes = await readFile(reservationPath);
  const reservation = await checkFreshBlindReservationIdentity({
    candidateSha,
    fixturePath,
    releaseId,
    reservationPath,
    runAttempt: required("GITHUB_RUN_ATTEMPT"),
    runId: required("GITHUB_RUN_ID"),
  });
  const manifestBytes = await readFile(manifestPath);
  const manifest = parseFreshBlindPreflightManifest(
    JSON.parse(manifestBytes.toString("utf8")) as unknown,
  );
  let started: FreshBlindStartedReceipt;
  const startedPath = freshBlindStartedReceiptPath(receiptPath);
  if (existsSync(startedPath)) {
    const current = parseFreshBlindReceipt(
      JSON.parse(await readFile(startedPath, "utf8")) as unknown,
    );
    if (current.status !== "started") {
      throw new Error("fresh blind started receipt path is not started");
    }
    started = current;
  } else {
    started = createFreshBlindStartedReceipt({
      manifest,
      manifestSha256: sha256(manifestBytes),
      reservation,
      reservationSha256: sha256(reservationBytes),
      startedAt: new Date().toISOString(),
    });
    await reserveFreshBlindReceipt(receiptPath, started);
  }
  const terminal = createFreshBlindExecutionErrorReceipt(
    started,
    `reserved execution did not produce a terminal receipt (workflow step outcome: ${required("SEMATH_RELEASE_EXECUTION_OUTCOME")})`,
  );
  await finalizeFreshBlindReceipt(receiptPath, terminal);
  console.log("fresh blind execution-error receipt finalized");
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
