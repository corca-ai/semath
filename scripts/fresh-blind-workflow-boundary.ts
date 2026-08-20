export const FRESH_BLIND_REPOSITORY = "corca-ai/semath";
export const FRESH_BLIND_LEDGER_ISSUE = "354";
export const FRESH_BLIND_WORKFLOW_REF =
  "corca-ai/semath/.github/workflows/fresh-blind-release.yml@refs/heads/main";

export interface FreshBlindWorkflowBoundary {
  readonly actions: string;
  readonly candidateSha: string;
  readonly ledgerIssue: string;
  readonly repository: string;
  readonly runAttempt: string;
  readonly runId: string;
  readonly workflowRef: string;
  readonly workflowSha: string;
}

export function assertFreshBlindWorkflowBoundary(
  boundary: FreshBlindWorkflowBoundary,
): void {
  if (boundary.actions !== "true") {
    throw new Error("fresh blind release requires GitHub Actions");
  }
  if (boundary.repository !== FRESH_BLIND_REPOSITORY) {
    throw new Error("fresh blind release requires the official repository");
  }
  if (boundary.ledgerIssue !== FRESH_BLIND_LEDGER_ISSUE) {
    throw new Error(
      "fresh blind release requires the permanent release ledger",
    );
  }
  if (boundary.workflowRef !== FRESH_BLIND_WORKFLOW_REF) {
    throw new Error("fresh blind release requires the reviewed main workflow");
  }
  for (const [label, value] of [
    ["candidate SHA", boundary.candidateSha],
    ["workflow SHA", boundary.workflowSha],
  ] as const) {
    if (!/^[0-9a-f]{40}$/u.test(value)) {
      throw new Error(
        `fresh blind ${label} must be a full lowercase commit SHA`,
      );
    }
  }
  for (const [label, value] of [
    ["run id", boundary.runId],
    ["run attempt", boundary.runAttempt],
  ] as const) {
    if (!/^[1-9][0-9]*$/u.test(value)) {
      throw new Error(`fresh blind ${label} is invalid`);
    }
  }
}

export function freshBlindWorkflowBoundaryFromEnvironment(
  candidateSha: string,
): FreshBlindWorkflowBoundary {
  return {
    actions: required("GITHUB_ACTIONS"),
    candidateSha,
    ledgerIssue: required("SEMATH_RELEASE_LEDGER_ISSUE"),
    repository: required("GITHUB_REPOSITORY"),
    runAttempt: required("GITHUB_RUN_ATTEMPT"),
    runId: required("GITHUB_RUN_ID"),
    workflowRef: required("GITHUB_WORKFLOW_REF"),
    workflowSha: required("GITHUB_WORKFLOW_SHA"),
  };
}

export function assertFreshBlindLinuxX64(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): void {
  if (platform !== "linux" || arch !== "x64") {
    throw new Error("fresh blind release requires Linux x86_64");
  }
}

function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return value;
}
