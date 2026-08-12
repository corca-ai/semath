import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { isAbsolute, relative, resolve } from "node:path";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  authoredFixtureSealPayload,
  authoredMathFingerprints,
  authoredProseShingles,
  authoredScenarioReviewPayload,
  freshBlindSealPayload,
  parseFreshBlindReleaseFixture,
  validateFreshBlindRelease,
  type AuthoredIntegrityProfile,
  type AuthoredLawCatalogEntry,
  type AuthoredScientificScenario,
  type FreshBlindReleaseFixture,
  type FreshBlindValidationSummary,
} from "../packages/evaluation/src/index";

export interface LoadedFreshBlindEvidence {
  readonly path: string;
  readonly release: FreshBlindReleaseFixture;
  readonly summary: FreshBlindValidationSummary;
}

export async function loadFreshBlindEvidence(
  explicitPath: string,
): Promise<LoadedFreshBlindEvidence> {
  if (!explicitPath.trim()) throw new Error("fresh blind fixture path is required");
  const path = isAbsolute(explicitPath)
    ? explicitPath
    : resolve(process.cwd(), explicitPath);
  const release = parseFreshBlindReleaseFixture(
    JSON.parse(await readFile(path, "utf8")),
  );
  const freshProfiles = release.fixture.scenarios.map(scenarioProfile);
  const freshIsolationProfiles = release.fixture.scenarios.flatMap((scenario) => [
    scenarioProfile(scenario),
    ...scenario.snapshots.flatMap((snapshot) =>
      snapshot.documents.map((document) =>
        documentProfile({
          content: document.content,
          id: `${scenario.id}/${snapshot.id}/${document.fileId}`,
          path: document.path,
        }),
      ),
    ),
  ]);
  const references = await referenceEvidence(path);
  const reviewDigests = Object.fromEntries(
    release.fixture.scenarios.map((scenario) => [
      scenario.id,
      sha256(authoredScenarioReviewPayload(release.fixture, scenario.id)),
    ]),
  );
  const summary = validateFreshBlindRelease(release, {
    authoredSealDigest: sha256(authoredFixtureSealPayload(release.fixture)),
    freshIsolationProfiles,
    freshProfiles,
    lawCatalog: await readLawCatalog(),
    referenceDocuments: references.documents,
    referenceProfiles: references.profiles,
    reviewDigests,
    sealDigest: sha256(freshBlindSealPayload(release)),
  });
  return { path, release, summary };
}

export function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

async function readLawCatalog(): Promise<AuthoredLawCatalogEntry[]> {
  const catalog: AuthoredLawCatalogEntry[] = [];
  for (const path of [...new Bun.Glob("packs/*/v1.json").scanSync(".")].sort()) {
    const value = JSON.parse(await readFile(path, "utf8")) as {
      readonly packId: string;
      readonly laws: readonly {
        readonly id: string;
        readonly roles: readonly (
          | string
          | { readonly id: string; readonly variadic?: boolean }
        )[];
      }[];
    };
    for (const law of value.laws) {
      catalog.push({
        field: value.packId,
        lawId: `${value.packId}:${law.id}`,
        roles: law.roles.map((role) =>
          typeof role === "string"
            ? { id: role, variadic: false }
            : { id: role.id, variadic: role.variadic === true },
        ),
      });
    }
  }
  return catalog;
}

async function referenceEvidence(
  freshPath: string,
): Promise<{
  readonly documents: readonly string[];
  readonly profiles: readonly AuthoredIntegrityProfile[];
}> {
  const documents = new Map<string, ReferenceDocument>();
  for await (const fixturePath of new Bun.Glob("fixtures/**/*.json").scan(".")) {
    if (resolve(fixturePath) === freshPath) continue;
    collectDocuments(
      JSON.parse(await readFile(fixturePath, "utf8")),
      fixturePath,
      documents,
    );
  }
  const values = [...documents.values()];
  return {
    documents: values.map((document) => document.content),
    profiles: values.map(documentProfile),
  };
}

interface ReferenceDocument {
  readonly content: string;
  readonly id: string;
  readonly path: string;
}

function collectDocuments(
  value: unknown,
  fixturePath: string,
  output: Map<string, ReferenceDocument>,
): void {
  if (Array.isArray(value)) {
    for (const child of value) collectDocuments(child, fixturePath, output);
    return;
  }
  if (typeof value !== "object" || value === null) return;
  const item = value as Record<string, unknown>;
  if (typeof item.content === "string") {
    const contentDigest = sha256(item.content);
    if (!output.has(contentDigest)) {
      output.set(contentDigest, {
        content: item.content,
        id: `${relative(process.cwd(), fixturePath)}:${contentDigest.slice(0, 12)}`,
        path: typeof item.path === "string" ? item.path : "fixture.md",
      });
    }
  }
  for (const child of Object.values(item)) {
    collectDocuments(child, fixturePath, output);
  }
}

function scenarioProfile(
  scenario: AuthoredScientificScenario,
): AuthoredIntegrityProfile {
  const math = new Set<string>();
  const prose = new Set<string>();
  for (const snapshot of scenario.snapshots) {
    const service = new LatexSyntaxService();
    service.reset({
      documents: snapshot.documents.map((document) => ({
        ...document,
        documentVersion: 1,
        language: languageOf(document.path),
      })),
    });
    for (const document of snapshot.documents) {
      const syntax = service.getFile(document.fileId);
      if (!syntax) throw new Error(`${scenario.id}: missing wasmtex syntax`);
      for (const fingerprint of authoredMathFingerprints(syntax)) math.add(fingerprint);
      for (const shingle of authoredProseShingles(document.content, syntax)) prose.add(shingle);
    }
  }
  return {
    id: scenario.id,
    mathFingerprints: [...math].sort(),
    proseShingles: [...prose].sort(),
  };
}

function documentProfile(document: ReferenceDocument): AuthoredIntegrityProfile {
  const service = new LatexSyntaxService();
  service.reset({
    documents: [
      {
        content: document.content,
        documentVersion: 1,
        fileId: document.id,
        language: languageOf(document.path),
        path: document.path,
      },
    ],
  });
  const syntax = service.getFile(document.id);
  if (!syntax) throw new Error(`${document.id}: missing wasmtex syntax`);
  return {
    id: document.id,
    mathFingerprints: authoredMathFingerprints(syntax),
    proseShingles: authoredProseShingles(document.content, syntax),
  };
}

function languageOf(path: string): "latex" | "markdown" {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
