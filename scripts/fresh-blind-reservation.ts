import { spawnSync } from "node:child_process";
import { open, mkdir, writeFile } from "node:fs/promises";
import { dirname, extname } from "node:path";
import { loadFreshBlindEvidence, sha256 } from "./fresh-blind-evidence";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  FRESH_BLIND_LEDGER_ISSUE,
  FRESH_BLIND_REPOSITORY,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

const MARKER =
  /<!-- semath-fresh-blind-reservation:(v0\.[1-9][0-9]*):([0-9a-f]{64}):([0-9a-f]{40}) -->/gu;
const HISTORICAL_SPENT_MARKER =
  /<!-- semath-fresh-blind:(v0\.[1-9][0-9]*):spent -->/gu;
const TERMINAL_MARKER =
  /<!-- semath-fresh-blind-result:(v0\.[1-9][0-9]*):[0-9a-f]{40}:(?:completed|safety-failed|execution-error):(?:[0-9a-f]{64}|none) -->/gu;

export interface FreshBlindReservationIdentity {
  readonly candidateSha: string;
  readonly fixtureSeal: string;
  readonly releaseId: string;
  readonly runAttempt: string;
  readonly runId: string;
}

export function freshBlindReservationMarker(
  identity: FreshBlindReservationIdentity,
): string {
  validateIdentity(identity);
  return `<!-- semath-fresh-blind-reservation:${identity.releaseId}:${identity.fixtureSeal}:${identity.candidateSha} -->`;
}

export function assertFreshBlindIdentityAvailable(
  bodies: readonly string[],
  identity: FreshBlindReservationIdentity,
): void {
  validateIdentity(identity);
  for (const body of bodies) {
    for (const match of body.matchAll(HISTORICAL_SPENT_MARKER)) {
      if (match[1] === identity.releaseId) {
        throw new Error(
          `fresh blind release id is already spent: ${identity.releaseId}`,
        );
      }
    }
    for (const match of body.matchAll(TERMINAL_MARKER)) {
      if (match[1] === identity.releaseId) {
        throw new Error(
          `fresh blind release id is already spent: ${identity.releaseId}`,
        );
      }
    }
    for (const match of body.matchAll(MARKER)) {
      if (match[1] === identity.releaseId) {
        throw new Error(
          `fresh blind release id is already spent: ${identity.releaseId}`,
        );
      }
      if (match[2] === identity.fixtureSeal) {
        throw new Error("fresh blind fixture seal is already spent");
      }
    }
  }
}

if (import.meta.main) await reserve();

async function reserve(): Promise<void> {
  assertFreshBlindLinuxX64();
  const candidateSha = required("SEMATH_CANDIDATE_SHA");
  const boundary = freshBlindWorkflowBoundaryFromEnvironment(candidateSha);
  assertFreshBlindWorkflowBoundary(boundary);
  assertCandidateCheckout(candidateSha, boundary.workflowSha);
  const fixturePath = required("SEMATH_FRESH_BLIND_FIXTURE");
  const evidence = await loadFreshBlindEvidence(fixturePath);
  const identity: FreshBlindReservationIdentity = {
    candidateSha,
    fixtureSeal: evidence.release.release.seal,
    releaseId: required("SEMATH_RELEASE_ID"),
    runAttempt: required("GITHUB_RUN_ATTEMPT"),
    runId: required("GITHUB_RUN_ID"),
  };
  validateIdentity(identity);
  if (identity.releaseId !== evidence.release.release.id) {
    throw new Error("reservation release id does not match the sealed fixture");
  }
  const repository = required("GITHUB_REPOSITORY");
  const issue = required("SEMATH_RELEASE_LEDGER_ISSUE");
  if (
    repository !== FRESH_BLIND_REPOSITORY ||
    issue !== FRESH_BLIND_LEDGER_ISSUE
  ) {
    throw new Error("reservation must use the official permanent ledger");
  }
  const token = required("GITHUB_TOKEN");
  const bodies = await issueCommentBodies(repository, issue, token);
  assertFreshBlindIdentityAvailable(bodies, identity);
  const marker = freshBlindReservationMarker(identity);
  const commentBody =
    `${marker}\nReserved ${identity.releaseId} for one execution at candidate ` +
    `\`${identity.candidateSha}\` (workflow run ${identity.runId}, attempt ${identity.runAttempt}). ` +
    `Fixture seal: \`${identity.fixtureSeal}\`. Any reservation is permanently spent.`;
  const response = await fetch(
    `https://api.github.com/repos/${repository}/issues/${issue}/comments`,
    {
      body: JSON.stringify({
        body: commentBody,
      }),
      headers: githubHeaders(token),
      method: "POST",
    },
  );
  if (!response.ok) {
    throw new Error(
      `failed to reserve release identity: GitHub ${response.status}`,
    );
  }
  const comment = parseFreshBlindLedgerComment(await response.json());
  assertFreshBlindLedgerComment(comment, {
    marker,
    repository,
    issue,
  });
  const reservation = {
    ...identity,
    ledgerCommentId: comment.id,
    marker,
    reservedAt: comment.createdAt,
    schemaVersion: 1,
  } as const;
  const bytes = `${JSON.stringify(reservation, null, 2)}\n`;
  const digest = sha256(bytes);
  const path = required("SEMATH_FRESH_BLIND_RESERVATION");
  await mkdir(dirname(path), { recursive: true });
  await writeExclusive(contentAddressedPath(path, digest), bytes);
  await writeFile(`${path}.sha256`, `${digest}  ${path.split("/").at(-1)}\n`, {
    flag: "wx",
  });
  // Publish the canonical reservation only after its immutable addressed copy
  // and checksum sidecar are durable.
  await writeExclusive(path, bytes);
  console.log(`fresh blind reservation recorded: ${digest}`);
}

export interface FreshBlindLedgerComment {
  readonly author: string;
  readonly body: string;
  readonly createdAt: string;
  readonly htmlUrl: string;
  readonly id: string;
  readonly issueUrl: string;
}

export function parseFreshBlindLedgerComment(
  value: unknown,
): FreshBlindLedgerComment {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("release ledger comment must be an object");
  }
  const item = value as Record<string, unknown>;
  const user = item.user;
  if (typeof user !== "object" || user === null || Array.isArray(user)) {
    throw new Error("release ledger comment user must be an object");
  }
  const id = item.id;
  if (typeof id !== "number" || !Number.isSafeInteger(id) || id <= 0) {
    throw new Error("release ledger comment id is invalid");
  }
  return {
    author: stringField(
      user as Record<string, unknown>,
      "login",
      "release ledger comment user",
    ),
    body: stringField(item, "body", "release ledger comment"),
    createdAt: stringField(item, "created_at", "release ledger comment"),
    htmlUrl: stringField(item, "html_url", "release ledger comment"),
    id: String(id),
    issueUrl: stringField(item, "issue_url", "release ledger comment"),
  };
}

export function assertFreshBlindLedgerComment(
  comment: FreshBlindLedgerComment,
  expected: {
    readonly issue: string;
    readonly marker: string;
    readonly repository: string;
  },
): void {
  if (comment.author !== "github-actions[bot]") {
    throw new Error("release reservation must be authored by GitHub Actions");
  }
  if (!comment.body.includes(expected.marker)) {
    throw new Error("release ledger comment is missing the reservation marker");
  }
  if (
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u.test(
      comment.createdAt,
    )
  ) {
    throw new Error("release ledger comment timestamp is invalid");
  }
  const issueApi = `https://api.github.com/repos/${expected.repository}/issues/${expected.issue}`;
  const issueWeb = `https://github.com/${expected.repository}/issues/${expected.issue}`;
  if (
    comment.issueUrl !== issueApi ||
    comment.htmlUrl !== `${issueWeb}#issuecomment-${comment.id}`
  ) {
    throw new Error(
      "release ledger comment URL does not match the permanent ledger",
    );
  }
}

function stringField(
  value: Record<string, unknown>,
  key: string,
  label: string,
): string {
  const field = value[key];
  if (typeof field !== "string" || !field.trim()) {
    throw new Error(`${label} ${key} must be a non-empty string`);
  }
  return field;
}

async function writeExclusive(path: string, bytes: string): Promise<void> {
  const file = await open(path, "wx");
  try {
    await file.writeFile(bytes);
    await file.sync();
  } finally {
    await file.close();
  }
}

function contentAddressedPath(path: string, digest: string): string {
  const extension = extname(path);
  return extension
    ? `${path.slice(0, -extension.length)}.${digest}${extension}`
    : `${path}.${digest}`;
}

async function issueCommentBodies(
  repository: string,
  issue: string,
  token: string,
): Promise<string[]> {
  const bodies: string[] = [];
  for (let page = 1; ; page += 1) {
    const response = await fetch(
      `https://api.github.com/repos/${repository}/issues/${issue}/comments?per_page=100&page=${page}`,
      { headers: githubHeaders(token) },
    );
    if (!response.ok) {
      throw new Error(
        `failed to read release ledger: GitHub ${response.status}`,
      );
    }
    const value: unknown = await response.json();
    if (!Array.isArray(value))
      throw new Error("release ledger response is not an array");
    const pageBodies = value.map((item) => {
      if (
        typeof item !== "object" ||
        item === null ||
        !("body" in item) ||
        typeof item.body !== "string"
      ) {
        throw new Error("release ledger comment has an invalid body");
      }
      return item.body;
    });
    bodies.push(...pageBodies);
    if (pageBodies.length < 100) return bodies;
  }
}

function githubHeaders(token: string): Record<string, string> {
  return {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${token}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
}

function validateIdentity(identity: FreshBlindReservationIdentity): void {
  if (!/^v0\.[1-9][0-9]*$/u.test(identity.releaseId)) {
    throw new Error("invalid fresh blind release id");
  }
  if (!/^[0-9a-f]{64}$/u.test(identity.fixtureSeal)) {
    throw new Error("invalid fresh blind fixture seal");
  }
  if (!/^[0-9a-f]{40}$/u.test(identity.candidateSha)) {
    throw new Error("invalid fresh blind candidate SHA");
  }
  if (
    !/^[1-9][0-9]*$/u.test(identity.runId) ||
    !/^[1-9][0-9]*$/u.test(identity.runAttempt)
  ) {
    throw new Error("invalid GitHub workflow identity");
  }
}

function assertCandidateCheckout(
  candidateSha: string,
  workflowSha: string,
): void {
  if (command("git", ["rev-parse", "HEAD"]) !== candidateSha) {
    throw new Error("reservation candidate SHA does not match HEAD");
  }
  if (command("git", ["status", "--porcelain"])) {
    throw new Error("reservation requires a clean candidate worktree");
  }
  command("git", ["merge-base", "--is-ancestor", workflowSha, candidateSha]);
}

function command(commandName: string, args: readonly string[]): string {
  const result = spawnSync(commandName, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(result.stderr || `${commandName} failed`);
  }
  return result.stdout.trim();
}

function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return value;
}
