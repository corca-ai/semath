import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import type { FreshBlindReservation } from "./check-fresh-blind-reservation";
import { freshBlindReservationMarker } from "./fresh-blind-reservation";
import {
  createFreshBlindExecutionErrorReceipt,
  createFreshBlindStartedReceipt,
  finalizeFreshBlindReceipt,
  freshBlindStartedReceiptPath,
  parseFreshBlindReceipt,
  planFreshBlindReceiptTransition,
  reserveFreshBlindReceipt,
  type FreshBlindStartedReceipt,
  type FreshBlindTerminalReceipt,
} from "./fresh-blind-receipt";
import {
  FRESH_BLIND_CONTRACTS,
  type FreshBlindPreflightManifest,
} from "./fresh-blind-preflight-manifest";

describe("fresh blind versioned receipt", () => {
  test("keeps immutable started and terminal records plus content-addressed copies", async () => {
    const directory = await mkdtemp(join(tmpdir(), "semath-fresh-receipt-"));
    const path = join(directory, "receipt.json");
    const started = startedReceipt();
    const terminal = completedReceipt(started);
    try {
      await reserveFreshBlindReceipt(path, started);
      await expect(reserveFreshBlindReceipt(path, started)).rejects.toThrow();
      await finalizeFreshBlindReceipt(path, terminal);
      expect(
        parseFreshBlindReceipt(
          JSON.parse(
            await readFile(freshBlindStartedReceiptPath(path), "utf8"),
          ) as unknown,
        ),
      ).toEqual(started);
      expect(
        parseFreshBlindReceipt(
          JSON.parse(await readFile(path, "utf8")) as unknown,
        ),
      ).toEqual(terminal);
      const files = await readdir(directory);
      expect(
        files.filter((file) => /receipt\.[0-9a-f]{64}\.json/u.test(file)),
      ).toHaveLength(1);
      expect(
        files.filter((file) =>
          /receipt\.started\.[0-9a-f]{64}\.json/u.test(file),
        ),
      ).toHaveLength(1);
      expect(files).toContain("receipt.json.sha256");
      expect(files).toContain("receipt.started.json.sha256");
    } finally {
      await rm(directory, { recursive: true });
    }
  });

  test("allows only identity-preserving terminal transitions", () => {
    const started = startedReceipt();
    const terminal = completedReceipt(started);
    expect(planFreshBlindReceiptTransition(started, terminal)).toEqual(
      terminal,
    );
    expect(() =>
      planFreshBlindReceiptTransition(started, {
        ...terminal,
        release: { ...terminal.release, fixtureId: "v0.38" },
      }),
    ).toThrow("reservation marker does not match");
    expect(() => planFreshBlindReceiptTransition(terminal, terminal)).toThrow(
      "started receipt",
    );
  });

  test("strictly rejects malformed contracts and terminal evidence", () => {
    const started = startedReceipt();
    expect(() =>
      parseFreshBlindReceipt({ ...started, schemaVersion: 2 }),
    ).toThrow("schemaVersion");
    expect(() => parseFreshBlindReceipt({ ...started, extra: true })).toThrow(
      "unexpected or missing fields",
    );
    const terminal = completedReceipt(started);
    expect(() =>
      parseFreshBlindReceipt({
        ...terminal,
        artifacts: { ...terminal.artifacts, lifecycleSha256: null },
      }),
    ).toThrow("requires evaluation and lifecycle");
    const result = terminal.result as Record<string, unknown>;
    const lifecycle = result.lifecycle as Record<string, unknown>;
    const lifecycleSubset = {
      ...terminal,
      result: {
        ...result,
        lifecycle: { ...lifecycle, comparedProbes: 8, comparedStages: 16 },
      },
    };
    expect(parseFreshBlindReceipt(lifecycleSubset)).toEqual(lifecycleSubset);
    expect(() =>
      parseFreshBlindReceipt({
        ...terminal,
        result: {
          ...result,
          authoringSafety: { cases: 0, failures: [] },
        },
      }),
    ).toThrow("authoringSafety.cases must be positive");
  });

  test("retains structured authoring safety findings in failed receipts", () => {
    const completed = completedReceipt(startedReceipt());
    const terminal = {
      ...completed,
      result: {
        ...(completed.result as Record<string, unknown>),
        authoringSafety: {
          cases: 48,
          failures: [{
            actual: "established",
            expected: "ambiguous",
            kind: "authority-escalation",
            path: "probe-01.authoringContext.disposition",
          }],
        },
        facetFailureIds: [
          "probe-01.authoringContext.disposition: authority-escalation",
        ],
      },
      status: "safety-failed" as const,
    };
    expect(parseFreshBlindReceipt(terminal)).toEqual(terminal);
  });

  test("continues to parse immutable policy-2 terminal receipts without authoringSafety", () => {
    const current = completedReceipt(startedReceipt());
    const result = { ...(current.result as Record<string, unknown>) };
    delete result.authoringSafety;
    const legacy = {
      ...current,
      contracts: { ...current.contracts, receiptPolicyVersion: 2 as const },
      receiptPolicyVersion: 2 as const,
      result,
      schemaVersion: 2 as const,
    };
    expect(parseFreshBlindReceipt(legacy)).toEqual(legacy);
  });

  test("requires structured authoring safety only in policy-3 terminals", () => {
    const current = completedReceipt(startedReceipt());
    const result = { ...(current.result as Record<string, unknown>) };
    delete result.authoringSafety;
    expect(() => parseFreshBlindReceipt({ ...current, result })).toThrow(
      "authoringSafety",
    );
    expect(current).toMatchObject({
      contracts: { receiptPolicyVersion: 3 },
      receiptPolicyVersion: 3,
      schemaVersion: 3,
    });
  });

  test("terminalizes a reserved execution without claiming evaluation evidence", () => {
    const started = startedReceipt();
    const terminal = createFreshBlindExecutionErrorReceipt(
      started,
      "runner failed",
    );
    expect(terminal.status).toBe("execution-error");
    expect(terminal.artifacts.evaluationSha256).toBeNull();
    expect(terminal.artifacts.lifecycleSha256).toBeNull();
    expect(terminal.result).toEqual({
      error: "runner failed",
      evaluation: null,
    });
  });
});

function startedReceipt(): FreshBlindStartedReceipt {
  const manifest = preflightManifest();
  const identity = {
    candidateSha: manifest.provenance.candidateCommit,
    fixtureSeal: manifest.release.fixtureSeal,
    releaseId: manifest.release.fixtureId,
    runAttempt: "1",
    runId: "123",
  } as const;
  const reservation: FreshBlindReservation = {
    ...identity,
    ledgerCommentId: "456",
    marker: freshBlindReservationMarker(identity),
    reservedAt: "2026-08-20T08:00:01.000Z",
    schemaVersion: 1,
  };
  return createFreshBlindStartedReceipt({
    manifest,
    manifestSha256: "7".repeat(64),
    reservation,
    reservationSha256: "8".repeat(64),
    startedAt: "2026-08-20T08:00:02.000Z",
  });
}

function completedReceipt(
  started: FreshBlindStartedReceipt,
): FreshBlindTerminalReceipt {
  return {
    ...started,
    artifacts: {
      ...started.artifacts,
      evaluationSha256: "9".repeat(64),
      lifecycleSha256: "a".repeat(64),
    },
    completedAt: "2026-08-20T08:01:00.000Z",
    result: {
      authoringSafety: {
        cases: 48,
        failures: [],
      },
      evaluation: { results: [{}] },
      facetFailureIds: [],
      lifecycle: {
        comparedProbes: 48,
        comparedStages: 96,
        fixtureId: "v0.37",
        fixtureSeal: "0".repeat(64),
        schemaVersion: 1,
      },
      safety: {
        diagnosticsOverLimit: 0,
        diagnosticsOverLimitIds: [],
        falseConflict: 0,
        falseConflictIds: [],
        falseEstablishment: 0,
        falseEstablishmentIds: [],
        unsafeNavigationOrEditCaseIds: [],
        unsafeNavigationOrEditLocations: 0,
      },
      validation: {
        decisions: { partial: 1 },
        families: { "single-document": 1 },
        laws: 1,
        maximumMathSimilarity: 0.2,
        maximumProseSimilarity: 0.3,
        probes: 1,
        scenarios: 1,
      },
    },
    status: "completed",
  };
}

function preflightManifest(): FreshBlindPreflightManifest {
  return {
    artifacts: {
      checksumManifestSha256: "1".repeat(64),
      committedWasmSha256: "2".repeat(64),
      nativeSha256: "3".repeat(64),
      npmTarballPath: ".artifacts/fresh-release/semath-0.18.0.tgz",
      npmTarballSha256: "4".repeat(64),
      rebuiltWasmSha256: "2".repeat(64),
      retainedChecksumPath: ".artifacts/fresh-release/SHA256SUMS",
      retainedWasmPath: ".artifacts/fresh-release/semath_wasm_bg.wasm",
    },
    contracts: FRESH_BLIND_CONTRACTS,
    gates: ["check"],
    generatedAt: "2026-08-20T08:00:00.000Z",
    provenance: {
      builderIdentity: "runner",
      candidateCommit: "5".repeat(40),
      candidateTree: "6".repeat(40),
      runnerArch: "X64",
      runnerImage: "ubuntu-24.04",
      runnerOs: "Linux",
      tools: { bun: "1.3.14", rust: "1.96.0", wasmBindgen: "0.2.100" },
      wasmtexCommit: "b".repeat(40),
      workflowFileSha256: "c".repeat(64),
      workflowRef: "workflow@main",
      workflowSha: "d".repeat(40),
    },
    references: {
      entries: [
        { path: "fixtures/development/public.json", sha256: "e".repeat(64) },
      ],
      sha256: "f".repeat(64),
    },
    release: {
      fixtureId: "v0.37",
      fixtureSeal: "0".repeat(64),
      fixtureSha256: "1".repeat(64),
      packageVersion: "0.18.0",
    },
    schemaVersion: 1,
  };
}
