import { spawnSync } from "node:child_process";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import {
  freshBlindReferenceManifest,
  isApprovedReferenceFixturePath,
  loadFreshBlindEvidence,
  sha256,
} from "./fresh-blind-evidence";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

export const FRESH_BLIND_RELEASE_CONTRACT = {
  packageVersion: "0.18.0",
  packSchemaVersion: 12,
  protocolVersion: 17,
  receiptPolicyVersion: 2,
  wasmtexSyntaxSchemaVersion: 8,
} as const;

export const FRESH_BLIND_CONTRACTS = {
  packSchemaVersion: 12,
  protocolVersion: 17,
  receiptPolicyVersion: 2,
  wasmtexSyntaxSchemaVersion: 8,
} as const;

export interface FreshBlindPreflightManifest {
  readonly artifacts: {
    readonly checksumManifestSha256: string;
    readonly committedWasmSha256: string;
    readonly nativeSha256: string;
    readonly npmTarballPath: string;
    readonly npmTarballSha256: string;
    readonly rebuiltWasmSha256: string;
    readonly retainedChecksumPath: string;
    readonly retainedWasmPath: string;
  };
  readonly contracts: {
    readonly packSchemaVersion: 12;
    readonly protocolVersion: 17;
    readonly receiptPolicyVersion: 2;
    readonly wasmtexSyntaxSchemaVersion: 8;
  };
  readonly gates: readonly string[];
  readonly generatedAt: string;
  readonly provenance: {
    readonly builderIdentity: string;
    readonly candidateCommit: string;
    readonly candidateTree: string;
    readonly runnerArch: "X64";
    readonly runnerImage: "ubuntu-24.04";
    readonly runnerOs: "Linux";
    readonly tools: {
      readonly bun: "1.3.14";
      readonly rust: "1.96.0";
      readonly wasmBindgen: "0.2.100";
    };
    readonly wasmtexCommit: string;
    readonly workflowFileSha256: string;
    readonly workflowRef: string;
    readonly workflowSha: string;
  };
  readonly references: {
    readonly entries: readonly {
      readonly path: string;
      readonly sha256: string;
    }[];
    readonly sha256: string;
  };
  readonly release: {
    readonly fixtureId: string;
    readonly fixtureSeal: string;
    readonly fixtureSha256: string;
    readonly packageVersion: "0.18.0";
  };
  readonly schemaVersion: 1;
}

export function buildFreshBlindPreflightManifest(
  input: FreshBlindPreflightManifest,
): FreshBlindPreflightManifest {
  return parseFreshBlindPreflightManifest(input);
}

export function assertFreshBlindWorkflowBytes(
  candidate: Uint8Array,
  executing: Uint8Array,
): void {
  if (sha256(candidate) !== sha256(executing)) {
    throw new Error(
      "candidate workflow differs from the reviewed executing workflow",
    );
  }
}

export function parseFreshBlindPreflightManifest(
  value: unknown,
): FreshBlindPreflightManifest {
  const root = record(value, "pre-blind manifest");
  exact(
    root,
    [
      "artifacts",
      "contracts",
      "gates",
      "generatedAt",
      "provenance",
      "references",
      "release",
      "schemaVersion",
    ],
    "pre-blind manifest",
  );
  literal(root.schemaVersion, 1, "pre-blind manifest.schemaVersion");
  const generatedAt = string(
    root.generatedAt,
    "pre-blind manifest.generatedAt",
  );
  iso(generatedAt, "pre-blind manifest.generatedAt");

  const release = record(root.release, "pre-blind manifest.release");
  exact(
    release,
    ["fixtureId", "fixtureSeal", "fixtureSha256", "packageVersion"],
    "pre-blind manifest.release",
  );
  const fixtureId = releaseId(
    release.fixtureId,
    "pre-blind manifest.release.fixtureId",
  );
  const fixtureSeal = digest(
    release.fixtureSeal,
    "pre-blind manifest.release.fixtureSeal",
  );
  const fixtureSha256 = digest(
    release.fixtureSha256,
    "pre-blind manifest.release.fixtureSha256",
  );
  literal(
    release.packageVersion,
    FRESH_BLIND_RELEASE_CONTRACT.packageVersion,
    "pre-blind manifest.release.packageVersion",
  );

  const contracts = record(root.contracts, "pre-blind manifest.contracts");
  exact(
    contracts,
    [
      "packSchemaVersion",
      "protocolVersion",
      "receiptPolicyVersion",
      "wasmtexSyntaxSchemaVersion",
    ],
    "pre-blind manifest.contracts",
  );
  literal(
    contracts.protocolVersion,
    17,
    "pre-blind manifest.contracts.protocolVersion",
  );
  literal(
    contracts.packSchemaVersion,
    12,
    "pre-blind manifest.contracts.packSchemaVersion",
  );
  literal(
    contracts.wasmtexSyntaxSchemaVersion,
    8,
    "pre-blind manifest.contracts.wasmtexSyntaxSchemaVersion",
  );
  literal(
    contracts.receiptPolicyVersion,
    2,
    "pre-blind manifest.contracts.receiptPolicyVersion",
  );

  const provenance = record(root.provenance, "pre-blind manifest.provenance");
  exact(
    provenance,
    [
      "builderIdentity",
      "candidateCommit",
      "candidateTree",
      "runnerArch",
      "runnerImage",
      "runnerOs",
      "tools",
      "wasmtexCommit",
      "workflowFileSha256",
      "workflowRef",
      "workflowSha",
    ],
    "pre-blind manifest.provenance",
  );
  const tools = record(provenance.tools, "pre-blind manifest.provenance.tools");
  exact(
    tools,
    ["bun", "rust", "wasmBindgen"],
    "pre-blind manifest.provenance.tools",
  );
  literal(tools.bun, "1.3.14", "pre-blind manifest.provenance.tools.bun");
  literal(tools.rust, "1.96.0", "pre-blind manifest.provenance.tools.rust");
  literal(
    tools.wasmBindgen,
    "0.2.100",
    "pre-blind manifest.provenance.tools.wasmBindgen",
  );
  literal(
    provenance.runnerOs,
    "Linux",
    "pre-blind manifest.provenance.runnerOs",
  );
  literal(
    provenance.runnerArch,
    "X64",
    "pre-blind manifest.provenance.runnerArch",
  );
  literal(
    provenance.runnerImage,
    "ubuntu-24.04",
    "pre-blind manifest.provenance.runnerImage",
  );
  const candidateCommit = commit(
    provenance.candidateCommit,
    "pre-blind manifest.provenance.candidateCommit",
  );
  const candidateTree = commit(
    provenance.candidateTree,
    "pre-blind manifest.provenance.candidateTree",
  );
  const wasmtexCommit = commit(
    provenance.wasmtexCommit,
    "pre-blind manifest.provenance.wasmtexCommit",
  );
  const workflowSha = commit(
    provenance.workflowSha,
    "pre-blind manifest.provenance.workflowSha",
  );
  const workflowFileSha256 = digest(
    provenance.workflowFileSha256,
    "pre-blind manifest.provenance.workflowFileSha256",
  );
  const workflowRef = nonempty(
    provenance.workflowRef,
    "pre-blind manifest.provenance.workflowRef",
  );
  const builderIdentity = nonempty(
    provenance.builderIdentity,
    "pre-blind manifest.provenance.builderIdentity",
  );

  const artifacts = record(root.artifacts, "pre-blind manifest.artifacts");
  exact(
    artifacts,
    [
      "checksumManifestSha256",
      "committedWasmSha256",
      "nativeSha256",
      "npmTarballPath",
      "npmTarballSha256",
      "rebuiltWasmSha256",
      "retainedChecksumPath",
      "retainedWasmPath",
    ],
    "pre-blind manifest.artifacts",
  );
  const committedWasmSha256 = digest(
    artifacts.committedWasmSha256,
    "pre-blind manifest.artifacts.committedWasmSha256",
  );
  const rebuiltWasmSha256 = digest(
    artifacts.rebuiltWasmSha256,
    "pre-blind manifest.artifacts.rebuiltWasmSha256",
  );
  if (committedWasmSha256 !== rebuiltWasmSha256)
    throw new Error("pre-blind manifest: committed and rebuilt WASM differ");

  const references = parseReferences(root.references);
  const gates = stringArray(root.gates, "pre-blind manifest.gates");
  if (gates.length === 0 || new Set(gates).size !== gates.length)
    throw new Error("pre-blind manifest.gates must be a non-empty unique list");

  return {
    artifacts: {
      checksumManifestSha256: digest(
        artifacts.checksumManifestSha256,
        "pre-blind manifest.artifacts.checksumManifestSha256",
      ),
      committedWasmSha256,
      nativeSha256: digest(
        artifacts.nativeSha256,
        "pre-blind manifest.artifacts.nativeSha256",
      ),
      npmTarballPath: nonempty(
        artifacts.npmTarballPath,
        "pre-blind manifest.artifacts.npmTarballPath",
      ),
      npmTarballSha256: digest(
        artifacts.npmTarballSha256,
        "pre-blind manifest.artifacts.npmTarballSha256",
      ),
      rebuiltWasmSha256,
      retainedChecksumPath: nonempty(
        artifacts.retainedChecksumPath,
        "pre-blind manifest.artifacts.retainedChecksumPath",
      ),
      retainedWasmPath: nonempty(
        artifacts.retainedWasmPath,
        "pre-blind manifest.artifacts.retainedWasmPath",
      ),
    },
    contracts: FRESH_BLIND_CONTRACTS,
    gates,
    generatedAt,
    provenance: {
      builderIdentity,
      candidateCommit,
      candidateTree,
      runnerArch: "X64",
      runnerImage: "ubuntu-24.04",
      runnerOs: "Linux",
      tools: { bun: "1.3.14", rust: "1.96.0", wasmBindgen: "0.2.100" },
      wasmtexCommit,
      workflowFileSha256,
      workflowRef,
      workflowSha,
    },
    references,
    release: {
      fixtureId,
      fixtureSeal,
      fixtureSha256,
      packageVersion: "0.18.0",
    },
    schemaVersion: 1,
  };
}

if (import.meta.main) await writeManifestFromEnvironment();

async function writeManifestFromEnvironment(): Promise<void> {
  assertFreshBlindLinuxX64();
  const candidateCommit = required("SEMATH_CANDIDATE_SHA");
  const boundary = freshBlindWorkflowBoundaryFromEnvironment(candidateCommit);
  assertFreshBlindWorkflowBoundary(boundary);
  if (command("git", ["rev-parse", "HEAD"]) !== candidateCommit)
    throw new Error("pre-blind candidate does not match HEAD");
  if (command("git", ["status", "--porcelain"]))
    throw new Error("pre-blind manifest requires a clean worktree");
  const fixturePath = requiredPath("SEMATH_FRESH_BLIND_FIXTURE");
  const fixtureBytes = await readFile(fixturePath);
  const evidence = await loadFreshBlindEvidence(fixturePath);
  const releaseIdValue = required("SEMATH_RELEASE_ID");
  if (evidence.release.release.id !== releaseIdValue)
    throw new Error("pre-blind release id does not match fixture");
  const packageManifest = parsePackageManifest(
    JSON.parse(await readFile("package.json", "utf8")) as unknown,
  );
  await assertReleaseFreeze(packageManifest);
  assertUnusedReleaseIdentities(releaseIdValue, packageManifest.version);
  assertToolVersion(command("bun", ["--version"]), "1.3.14", "Bun");
  assertToolVersion(
    command("rustc", ["--version"]).split(" ")[1] ?? "",
    "1.96.0",
    "Rust",
  );
  assertToolVersion(
    command("wasm-bindgen", ["--version"]).split(" ").at(-1) ?? "",
    "0.2.100",
    "wasm-bindgen",
  );

  const manifestPath = requiredPath("SEMATH_FRESH_BLIND_PREFLIGHT_MANIFEST");
  const tarballPath = requiredPath("SEMATH_FRESH_BLIND_PACKAGE");
  await mkdir(dirname(manifestPath), { recursive: true });
  await retainPackage(tarballPath, packageManifest);
  const retainedWasmPath = resolve(
    dirname(manifestPath),
    "semath_wasm_bg.wasm",
  );
  const retainedChecksumPath = resolve(dirname(manifestPath), "SHA256SUMS");
  await copyFile("lib/wasm/semath_wasm_bg.wasm", retainedWasmPath);
  await copyFile("lib/wasm/SHA256SUMS", retainedChecksumPath);

  const rebuiltWasm = await readFile("lib/wasm/semath_wasm_bg.wasm");
  const committedWasm = commandBytes("git", [
    "show",
    `${candidateCommit}:lib/wasm/semath_wasm_bg.wasm`,
  ]);
  const references = await freshBlindReferenceManifest();
  const workflowPath = ".github/workflows/fresh-blind-release.yml";
  command("git", [
    "merge-base",
    "--is-ancestor",
    boundary.workflowSha,
    candidateCommit,
  ]);
  const candidateWorkflow = await readFile(workflowPath);
  const executingWorkflow = commandBytes("git", [
    "show",
    `${boundary.workflowSha}:${workflowPath}`,
  ]);
  assertFreshBlindWorkflowBytes(candidateWorkflow, executingWorkflow);
  const manifest = buildFreshBlindPreflightManifest({
    artifacts: {
      checksumManifestSha256: sha256(await readFile("lib/wasm/SHA256SUMS")),
      committedWasmSha256: sha256(committedWasm),
      nativeSha256: sha256(await readFile("target/debug/semath-native")),
      npmTarballPath: relativeArtifactPath(tarballPath),
      npmTarballSha256: sha256(await readFile(tarballPath)),
      rebuiltWasmSha256: sha256(rebuiltWasm),
      retainedChecksumPath: relativeArtifactPath(retainedChecksumPath),
      retainedWasmPath: relativeArtifactPath(retainedWasmPath),
    },
    contracts: FRESH_BLIND_CONTRACTS,
    gates: [
      "check",
      "quality",
      "authored-development",
      "docs",
      "committed-wasm",
      "package-smoke",
      "continuity",
      "authored-historical",
      "fresh-static-validation",
    ],
    generatedAt: new Date().toISOString(),
    provenance: {
      builderIdentity: required("SEMATH_RELEASE_BUILDER_IDENTITY"),
      candidateCommit,
      candidateTree: command("git", ["rev-parse", "HEAD^{tree}"]),
      runnerArch: "X64",
      runnerImage: "ubuntu-24.04",
      runnerOs: "Linux",
      tools: { bun: "1.3.14", rust: "1.96.0", wasmBindgen: "0.2.100" },
      wasmtexCommit: packageManifest.wasmtexCommit,
      workflowFileSha256: sha256(executingWorkflow),
      workflowRef: boundary.workflowRef,
      workflowSha: boundary.workflowSha,
    },
    references,
    release: {
      fixtureId: evidence.release.release.id,
      fixtureSeal: evidence.release.release.seal,
      fixtureSha256: sha256(fixtureBytes),
      packageVersion: "0.18.0",
    },
    schemaVersion: 1,
  });
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, {
    flag: "wx",
  });
  console.log(
    `pre-blind manifest recorded: ${sha256(await readFile(manifestPath))}`,
  );
}

interface ParsedPackageManifest {
  readonly name: "semath";
  readonly version: "0.18.0";
  readonly wasmtexCommit: string;
}

function parsePackageManifest(value: unknown): ParsedPackageManifest {
  const item = record(value, "package.json");
  literal(item.name, "semath", "package.json.name");
  literal(item.version, "0.18.0", "package.json.version");
  const dependencies = record(item.dependencies, "package.json.dependencies");
  const wasmtex = string(
    dependencies.wasmtex,
    "package.json.dependencies.wasmtex",
  );
  const wasmtexCommit = wasmtex.match(/#([0-9a-f]{40})$/u)?.[1];
  if (!wasmtexCommit)
    throw new Error("package.json must pin wasmtex to a full commit");
  const exports = record(item.exports, "package.json.exports");
  for (const key of ["./protocol", "./evaluation", "./wasm", "./wasm-binary"])
    nonempty(exports[key], `package.json.exports.${key}`);
  return { name: "semath", version: "0.18.0", wasmtexCommit };
}

async function assertReleaseFreeze(
  manifest: ParsedPackageManifest,
): Promise<void> {
  const cargoToml = await readFile("Cargo.toml", "utf8");
  const cargoLock = await readFile("Cargo.lock", "utf8");
  const rustProtocol = await readFile(
    "crates/semath-core/src/protocol.rs",
    "utf8",
  );
  const tsProtocol = await readFile("packages/protocol/src/index.ts", "utf8");
  if (!/^version = "0\.18\.0"$/mu.test(cargoToml))
    throw new Error("Cargo.toml package version must be 0.18.0");
  for (const name of ["semath-core", "semath-native", "semath-wasm"]) {
    const block = new RegExp(`name = "${name}"\\nversion = "0\\.18\\.0"`, "u");
    if (!block.test(cargoLock))
      throw new Error(`Cargo.lock ${name} version must be 0.18.0`);
  }
  if (
    !/PROTOCOL_VERSION: u32 = 17;/u.test(rustProtocol) ||
    !/SEMATH_PROTOCOL_VERSION = 17 as const/u.test(tsProtocol)
  )
    throw new Error("protocol version must be 17 in Rust and TypeScript");
  if (
    !/WASMTEX_SYNTAX_SCHEMA_VERSION: u32 = 8;/u.test(rustProtocol) ||
    !/WASMTEX_SYNTAX_SCHEMA_VERSION = 8 as const/u.test(tsProtocol)
  )
    throw new Error("wasmtex syntax schema must be 8");
  if (
    !/interpretations:\s*(?:readonly\s+)?MathInterpretationSetInfo/u.test(
      tsProtocol,
    )
  )
    throw new Error("protocol package is missing the interpretation surface");
  for (const path of [...new Bun.Glob("packs/*/v1.json").scanSync(".")]) {
    const pack = record(
      JSON.parse(await readFile(path, "utf8")) as unknown,
      path,
    );
    literal(pack.schemaVersion, 12, `${path}.schemaVersion`);
  }
  if (manifest.version !== FRESH_BLIND_RELEASE_CONTRACT.packageVersion)
    throw new Error("package version mismatch");
}

async function retainPackage(
  path: string,
  manifest: ParsedPackageManifest,
): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  command("bun", [
    "pm",
    "pack",
    "--destination",
    dirname(path),
    "--filename",
    path.split("/").at(-1) ?? "",
    "--ignore-scripts",
    "--quiet",
  ]);
  await readFile(path);
  const packaged = parsePackageManifest(
    JSON.parse(
      commandBytes("tar", ["-xOf", path, "package/package.json"]).toString(
        "utf8",
      ),
    ) as unknown,
  );
  if (
    packaged.version !== manifest.version ||
    packaged.wasmtexCommit !== manifest.wasmtexCommit
  )
    throw new Error("packed metadata differs from the frozen package");
  const packagedProtocol = commandBytes("tar", [
    "-xOf",
    path,
    "package/packages/protocol/src/index.ts",
  ]).toString("utf8");
  if (
    !/SEMATH_PROTOCOL_VERSION = 17 as const/u.test(packagedProtocol) ||
    !/interpretations:\s*(?:readonly\s+)?MathInterpretationSetInfo/u.test(
      packagedProtocol,
    )
  )
    throw new Error("packed protocol is missing protocol 17 interpretations");
  const packagedWasm = commandBytes("tar", [
    "-xOf",
    path,
    "package/lib/wasm/semath_wasm_bg.wasm",
  ]);
  if (
    sha256(packagedWasm) !==
    sha256(await readFile("lib/wasm/semath_wasm_bg.wasm"))
  )
    throw new Error("packed WASM differs from the committed artifact");
  await smokeRetainedPackage(path, manifest.version);
}

async function smokeRetainedPackage(
  path: string,
  version: string,
): Promise<void> {
  const temporary = await mkdtemp(join(tmpdir(), "semath-retained-package-"));
  try {
    await writeFile(
      join(temporary, "package.json"),
      `${JSON.stringify({ name: "semath-release-smoke", private: true, type: "module" })}\n`,
    );
    commandAt("bun", ["add", path], temporary);
    commandAt(
      "bun",
      [
        "-e",
        `const manifest = await Bun.file("node_modules/semath/package.json").json(); if (manifest.version !== "${version}") throw new Error("wrong installed version")`,
      ],
      temporary,
    );
    commandAt("bun", ["node_modules/semath/examples/worker.mjs"], temporary);
    commandAt("bun", ["node_modules/semath/examples/lsp.mjs"], temporary);
    const installedWasm = await readFile(
      join(temporary, "node_modules/semath/lib/wasm/semath_wasm_bg.wasm"),
    );
    if (
      sha256(installedWasm) !==
      sha256(await readFile("lib/wasm/semath_wasm_bg.wasm"))
    )
      throw new Error(
        "installed retained package WASM differs from the committed artifact",
      );
  } finally {
    await rm(temporary, { force: true, recursive: true });
  }
}

function assertUnusedReleaseIdentities(
  releaseIdValue: string,
  packageVersion: string,
): void {
  for (const tag of [
    releaseIdValue,
    `v${packageVersion}`,
    `semath-v${packageVersion}`,
  ]) {
    if (command("git", ["ls-remote", "--tags", "origin", `refs/tags/${tag}`]))
      throw new Error(`release tag is already used: ${tag}`);
  }
  const registry = spawnSync(
    "bun",
    ["pm", "view", `semath@${packageVersion}`, "version"],
    { encoding: "utf8" },
  );
  if (registry.status === 0 && registry.stdout.trim())
    throw new Error(
      `package version is already published: semath@${packageVersion}`,
    );
  if (
    registry.status !== 0 &&
    !/404|not found|E404/iu.test(`${registry.stdout}\n${registry.stderr}`)
  )
    throw new Error("could not prove the package version is unused");
}

function parseReferences(
  value: unknown,
): FreshBlindPreflightManifest["references"] {
  const item = record(value, "pre-blind manifest.references");
  exact(item, ["entries", "sha256"], "pre-blind manifest.references");
  if (!Array.isArray(item.entries) || item.entries.length === 0)
    throw new Error("pre-blind manifest.references.entries must be non-empty");
  const entries = item.entries.map((entry, index) => {
    const parsed = record(
      entry,
      `pre-blind manifest.references.entries[${index}]`,
    );
    exact(
      parsed,
      ["path", "sha256"],
      `pre-blind manifest.references.entries[${index}]`,
    );
    const path = nonempty(
      parsed.path,
      `pre-blind manifest.references.entries[${index}].path`,
    );
    if (!isApprovedReferenceFixturePath(path))
      throw new Error(
        "pre-blind manifest references a prohibited fixture namespace",
      );
    return {
      path,
      sha256: digest(
        parsed.sha256,
        `pre-blind manifest.references.entries[${index}].sha256`,
      ),
    };
  });
  const paths = entries.map((entry) => entry.path);
  if (
    new Set(paths).size !== paths.length ||
    JSON.stringify(paths) !== JSON.stringify([...paths].sort())
  )
    throw new Error(
      "pre-blind manifest reference entries must be unique and sorted",
    );
  const expected = sha256(`${JSON.stringify(entries)}\n`);
  const actual = digest(item.sha256, "pre-blind manifest.references.sha256");
  if (actual !== expected)
    throw new Error(
      "pre-blind manifest reference allowlist digest does not match entries",
    );
  return { entries, sha256: actual };
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
function literal<const T extends string | number>(
  value: unknown,
  expected: T,
  label: string,
): asserts value is T {
  if (value !== expected) throw new Error(`${label} must be ${expected}`);
}
function digest(value: unknown, label: string): string {
  const parsed = string(value, label);
  if (!/^[0-9a-f]{64}$/u.test(parsed))
    throw new Error(`${label} must be a SHA-256 digest`);
  return parsed;
}
function commit(value: unknown, label: string): string {
  const parsed = string(value, label);
  if (!/^[0-9a-f]{40}$/u.test(parsed))
    throw new Error(`${label} must be a full commit SHA`);
  return parsed;
}
function releaseId(value: unknown, label: string): string {
  const parsed = string(value, label);
  if (!/^v0\.[1-9][0-9]*$/u.test(parsed))
    throw new Error(`${label} must be a release id`);
  return parsed;
}
function iso(value: string, label: string): void {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u.test(value))
    throw new Error(`${label} must be an ISO UTC instant`);
}
function stringArray(value: unknown, label: string): string[] {
  if (
    !Array.isArray(value) ||
    value.some((item) => typeof item !== "string" || !item.trim())
  )
    throw new Error(`${label} must be a string array`);
  return [...value];
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
function command(commandName: string, args: readonly string[]): string {
  return commandBytes(commandName, args).toString("utf8").trim();
}
function commandBytes(commandName: string, args: readonly string[]): Buffer {
  const result = spawnSync(commandName, args);
  if (result.status !== 0)
    throw new Error(result.stderr?.toString() || `${commandName} failed`);
  return result.stdout;
}
function commandAt(
  commandName: string,
  args: readonly string[],
  cwd: string,
): string {
  const result = spawnSync(commandName, args, { cwd, encoding: "utf8" });
  if (result.status !== 0)
    throw new Error(result.stderr || result.stdout || `${commandName} failed`);
  return result.stdout.trim();
}
function assertToolVersion(
  actual: string,
  expected: string,
  name: string,
): void {
  if (actual !== expected)
    throw new Error(`${name} must be ${expected}; got ${actual}`);
}
function relativeArtifactPath(path: string): string {
  const relative = path.startsWith(`${process.cwd()}/`)
    ? path.slice(process.cwd().length + 1)
    : path;
  if (!relative || relative.startsWith(".."))
    throw new Error(
      "release artifact path must be inside the repository worktree",
    );
  return relative;
}
