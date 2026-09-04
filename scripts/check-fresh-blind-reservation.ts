import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { loadFreshBlindEvidence } from "./fresh-blind-evidence";
import {
  assertFreshBlindLedgerComment,
  freshBlindReservationMarker,
  parseFreshBlindLedgerComment,
  type FreshBlindReservationIdentity,
} from "./fresh-blind-reservation";
import {
  assertFreshBlindWorkflowBoundary,
  FRESH_BLIND_LEDGER_ISSUE,
  FRESH_BLIND_REPOSITORY,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

export interface FreshBlindReservation extends FreshBlindReservationIdentity {
  readonly ledgerCommentId: string;
  readonly marker: string;
  readonly reservedAt: string;
  readonly schemaVersion: 1;
}

export function parseFreshBlindReservation(
  value: unknown,
): FreshBlindReservation {
  if (!isRecord(value))
    throw new Error("fresh blind reservation must be an object");
  const expectedKeys = [
    "candidateSha",
    "fixtureSeal",
    "ledgerCommentId",
    "marker",
    "releaseId",
    "reservedAt",
    "runAttempt",
    "runId",
    "schemaVersion",
  ];
  const keys = Object.keys(value).sort();
  if (JSON.stringify(keys) !== JSON.stringify(expectedKeys)) {
    throw new Error("fresh blind reservation has unexpected or missing fields");
  }
  if (value.schemaVersion !== 1) {
    throw new Error("unsupported fresh blind reservation schema");
  }
  const candidateSha = stringField(value, "candidateSha");
  const fixtureSeal = stringField(value, "fixtureSeal");
  const marker = stringField(value, "marker");
  const ledgerCommentId = stringField(value, "ledgerCommentId");
  const releaseId = stringField(value, "releaseId");
  const reservedAt = stringField(value, "reservedAt");
  const runAttempt = stringField(value, "runAttempt");
  const runId = stringField(value, "runId");
  const identity: FreshBlindReservationIdentity = {
    candidateSha,
    fixtureSeal,
    releaseId,
    runAttempt,
    runId,
  };
  if (marker !== freshBlindReservationMarker(identity)) {
    throw new Error(
      "fresh blind reservation marker does not match its identity",
    );
  }
  if (!/^[1-9][0-9]*$/u.test(ledgerCommentId)) {
    throw new Error("fresh blind reservation ledgerCommentId is invalid");
  }
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u.test(reservedAt)) {
    throw new Error(
      "fresh blind reservation timestamp is not an ISO UTC instant",
    );
  }
  return { ...identity, ledgerCommentId, marker, reservedAt, schemaVersion: 1 };
}

export interface FreshBlindReservationCheck {
  readonly candidateSha: string;
  readonly fixturePath: string;
  readonly releaseId: string;
  readonly reservationPath: string;
  readonly runAttempt: string;
  readonly runId: string;
}

export interface FreshBlindPermanentReservationCheck
  extends FreshBlindReservationCheck {
  readonly githubToken: string;
}

export async function checkFreshBlindReservationIdentity(
  input: FreshBlindReservationCheck,
): Promise<FreshBlindReservation> {
  const reservation = parseFreshBlindReservation(
    JSON.parse(await readFile(input.reservationPath, "utf8")) as unknown,
  );
  const evidence = await loadFreshBlindEvidence(input.fixturePath);
  const expected = {
    candidateSha: input.candidateSha,
    fixtureSeal: evidence.release.release.seal,
    releaseId: input.releaseId,
    runAttempt: input.runAttempt,
    runId: input.runId,
  } satisfies FreshBlindReservationIdentity;
  assertFreshBlindReservationExecution(reservation, expected);
  return reservation;
}

export function assertFreshBlindReservationExecution(
  reservation: FreshBlindReservation,
  expected: FreshBlindReservationIdentity,
): void {
  for (const key of Object.keys(expected) as (keyof FreshBlindReservationIdentity)[]) {
    if (reservation[key] !== expected[key]) {
      throw new Error(
        `fresh blind reservation ${key} does not match execution`,
      );
    }
  }
}

export async function checkFreshBlindReservation(
  input: FreshBlindPermanentReservationCheck,
  request: typeof fetch = fetch,
): Promise<FreshBlindReservation> {
  const reservation = await checkFreshBlindReservationIdentity(input);
  await proveFreshBlindReservation(reservation, input.githubToken, request);
  return reservation;
}

export async function proveFreshBlindReservation(
  reservation: FreshBlindReservation,
  githubToken: string,
  request: typeof fetch = fetch,
): Promise<void> {
  const token = githubToken.trim();
  if (!token) throw new Error("GitHub token is required to prove the reservation");
  const response = await request(
    `https://api.github.com/repos/${FRESH_BLIND_REPOSITORY}/issues/comments/${reservation.ledgerCommentId}`,
    {
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${token}`,
        "X-GitHub-Api-Version": "2022-11-28",
      },
    },
  );
  if (!response.ok) {
    throw new Error(
      `could not prove the permanent release reservation: GitHub ${response.status}`,
    );
  }
  const comment = parseFreshBlindLedgerComment(await response.json());
  assertFreshBlindLedgerComment(comment, {
    issue: FRESH_BLIND_LEDGER_ISSUE,
    marker: reservation.marker,
    repository: FRESH_BLIND_REPOSITORY,
  });
  if (
    comment.id !== reservation.ledgerCommentId ||
    comment.createdAt !== reservation.reservedAt
  ) {
    throw new Error(
      "permanent release reservation comment differs from the local record",
    );
  }
}

if (import.meta.main) await checkReservationFromEnvironment();

async function checkReservationFromEnvironment(): Promise<void> {
  const args = Bun.argv.slice(2);
  if (args.length > 1 || (args.length === 1 && args[0] !== "--local")) {
    throw new Error("usage: check-fresh-blind-reservation.ts [--local]");
  }
  const candidateSha = required("SEMATH_CANDIDATE_SHA");
  assertFreshBlindWorkflowBoundary(
    freshBlindWorkflowBoundaryFromEnvironment(candidateSha),
  );
  const input: FreshBlindReservationCheck = {
    candidateSha,
    fixturePath: required("SEMATH_FRESH_BLIND_FIXTURE"),
    releaseId: required("SEMATH_RELEASE_ID"),
    reservationPath: requiredPath("SEMATH_FRESH_BLIND_RESERVATION"),
    runAttempt: required("GITHUB_RUN_ATTEMPT"),
    runId: required("GITHUB_RUN_ID"),
  };
  const reservation = args[0] === "--local"
    ? await checkFreshBlindReservationIdentity(input)
    : await checkFreshBlindReservation({
        ...input,
        githubToken: required("GITHUB_TOKEN"),
      });
  console.log(
    `fresh blind reservation ${args[0] === "--local" ? "identity " : ""}OK: ${reservation.releaseId}`,
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringField(value: Record<string, unknown>, key: string): string {
  const field = value[key];
  if (typeof field !== "string") {
    throw new Error(`fresh blind reservation ${key} must be a string`);
  }
  return field;
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
