import { describe, expect, test } from "bun:test";
import { sha256 } from "./fresh-blind-evidence";
import {
  assertFreshBlindWorkflowBytes,
  buildFreshBlindPreflightManifest,
  commandBytes,
  FRESH_BLIND_CONTRACTS,
  parseFreshBlindPreflightManifest,
  retainedPackagePackArguments,
  type FreshBlindPreflightManifest,
} from "./fresh-blind-preflight-manifest";

const references = [
  { path: "fixtures/development/public.json", sha256: "a".repeat(64) },
];

function manifest(): FreshBlindPreflightManifest {
  return {
    artifacts: {
      checksumManifestSha256: "b".repeat(64),
      committedWasmSha256: "c".repeat(64),
      nativeSha256: "d".repeat(64),
      npmTarballPath: ".artifacts/fresh-release/semath-0.18.0.tgz",
      npmTarballSha256: "e".repeat(64),
      rebuiltWasmSha256: "c".repeat(64),
      retainedChecksumPath: ".artifacts/fresh-release/SHA256SUMS",
      retainedWasmPath: ".artifacts/fresh-release/semath_wasm_bg.wasm",
    },
    contracts: FRESH_BLIND_CONTRACTS,
    gates: ["check", "quality", "fresh-static-validation"],
    generatedAt: "2026-08-20T08:00:00.000Z",
    provenance: {
      builderIdentity: "GitHub hosted runner / ubuntu24",
      candidateCommit: "f".repeat(40),
      candidateTree: "1".repeat(40),
      runnerArch: "X64",
      runnerImage: "ubuntu-24.04",
      runnerOs: "Linux",
      tools: { bun: "1.3.14", rust: "1.96.0", wasmBindgen: "0.2.100" },
      wasmtexCommit: "2".repeat(40),
      workflowFileSha256: "3".repeat(64),
      workflowRef:
        "corca-ai/semath/.github/workflows/fresh-blind-release.yml@refs/heads/main",
      workflowSha: "4".repeat(40),
    },
    references: {
      entries: references,
      sha256: sha256(`${JSON.stringify(references)}\n`),
    },
    release: {
      fixtureId: "v0.37",
      fixtureSeal: "5".repeat(64),
      fixtureSha256: "6".repeat(64),
      packageVersion: "0.18.0",
    },
    schemaVersion: 1,
  };
}

describe("fresh blind pre-blind manifest", () => {
  test("accepts one frozen candidate, contract, reference inventory, and artifact set", () => {
    expect(buildFreshBlindPreflightManifest(manifest())).toEqual(manifest());
    expect(manifest().contracts.receiptPolicyVersion).toBe(3);
  });

  test("rejects drift in contract versions, reference inventory, or rebuilt WASM", () => {
    expect(() =>
      parseFreshBlindPreflightManifest({
        ...manifest(),
        contracts: { ...manifest().contracts, protocolVersion: 16 },
      }),
    ).toThrow("protocolVersion");
    expect(() =>
      parseFreshBlindPreflightManifest({
        ...manifest(),
        contracts: { ...manifest().contracts, receiptPolicyVersion: 2 },
      }),
    ).toThrow("receiptPolicyVersion");
    expect(() =>
      parseFreshBlindPreflightManifest({
        ...manifest(),
        references: { ...manifest().references, sha256: "7".repeat(64) },
      }),
    ).toThrow("allowlist digest");
    const prohibited = [
      {
        path: "fixtures/challenge/document-reasoning-fresh-v036.json",
        sha256: "a".repeat(64),
      },
    ];
    expect(() =>
      parseFreshBlindPreflightManifest({
        ...manifest(),
        references: {
          entries: prohibited,
          sha256: sha256(`${JSON.stringify(prohibited)}\n`),
        },
      }),
    ).toThrow("prohibited fixture namespace");
    expect(() =>
      parseFreshBlindPreflightManifest({
        ...manifest(),
        artifacts: {
          ...manifest().artifacts,
          rebuiltWasmSha256: "8".repeat(64),
        },
      }),
    ).toThrow("committed and rebuilt WASM differ");
  });

  test("rejects unknown boundary fields", () => {
    expect(() =>
      parseFreshBlindPreflightManifest({ ...manifest(), surprise: true }),
    ).toThrow("unexpected or missing fields");
  });

  test("requires candidate and actually executing workflow bytes to match", () => {
    expect(() =>
      assertFreshBlindWorkflowBytes(
        new TextEncoder().encode("reviewed"),
        new TextEncoder().encode("reviewed"),
      ),
    ).not.toThrow();
    expect(() =>
      assertFreshBlindWorkflowBytes(
        new TextEncoder().encode("candidate"),
        new TextEncoder().encode("executing"),
      ),
    ).toThrow("candidate workflow differs");
  });

  test("packs the retained tarball at one exact path", () => {
    expect(
      retainedPackagePackArguments(
        ".artifacts/fresh-release/semath-0.18.0.tgz",
      ),
    ).toEqual([
      "pm",
      "pack",
      "--filename",
      ".artifacts/fresh-release/semath-0.18.0.tgz",
      "--ignore-scripts",
      "--quiet",
    ]);
  });

  test("retains binary command output larger than the spawnSync default", () => {
    const size = 2 * 1024 * 1024;
    const bytes = commandBytes(process.execPath, [
      "-e",
      `process.stdout.write(Buffer.alloc(${size}, 0x5a))`,
    ]);

    expect(bytes.byteLength).toBe(size);
    expect(bytes[0]).toBe(0x5a);
    expect(bytes.at(-1)).toBe(0x5a);
  });

  test("reports a bounded binary command overflow without hiding ENOBUFS", () => {
    const size = 9 * 1024 * 1024;

    expect(() =>
      commandBytes(process.execPath, [
        "-e",
        `process.stdout.write(Buffer.alloc(${size}, 0x5a))`,
      ]),
    ).toThrow("ENOBUFS");
  });
});
