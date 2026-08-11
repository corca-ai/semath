import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  authoredMathFingerprints,
  authoredProseShingles,
  authoredFixtureSealPayload,
  authoredScenarioReviewPayload,
  compareAuthoredIntegrityProfiles,
  parseAuthoredScientificFixture,
  validateAuthoredScientificTranche,
  type AuthoredIntegrityProfile,
  type AuthoredLawCatalogEntry,
  type AuthoredScientificFixture,
  type AuthoredScientificScenario,
} from "../packages/evaluation/src/index";

const fixturePaths = {
  development: new URL(
    "../fixtures/challenge/document-reasoning-development-v1.json",
    import.meta.url,
  ),
  holdout: new URL(
    "../fixtures/challenge/document-reasoning-holdout-v1.json",
    import.meta.url,
  ),
} as const;
const [development, holdout, lawCatalog] = await Promise.all([
  readFixture(fixturePaths.development),
  readFixture(fixturePaths.holdout),
  readLawCatalog(),
]);
const summary = validateAuthoredScientificTranche(
  development,
  holdout,
  lawCatalog,
  [
    "calculus-analysis",
    "discrete-math",
    "electromagnetism",
    "fluid-mechanics",
    "optimization-ml",
    "thermodynamics-heat-transfer",
  ],
);

verifyReviewDigests(development);
verifyReviewDigests(holdout);
if (
  holdout.batch.seal !==
  sha256(authoredFixtureSealPayload(holdout))
) {
  throw new Error("holdout batch seal does not cover the frozen fixture");
}
await rejectExternalHoldoutLeakage(holdout);
const integrity = verifyAuthoredSplitIntegrity(development, holdout);

console.log(
  [
    "authored scientific fixture: " + summary.developmentCases + " development",
    summary.holdoutCases + " frozen holdout",
    development.probes.length + holdout.probes.length + " probes",
    summary.laws + " laws",
    "max cross-split prose similarity " + integrity.maximumProse.toFixed(3),
  ].join(", "),
);

async function readFixture(
  path: URL,
): Promise<AuthoredScientificFixture> {
  return parseAuthoredScientificFixture(
    JSON.parse(await readFile(path, "utf8")),
  );
}

async function readLawCatalog(): Promise<AuthoredLawCatalogEntry[]> {
  const paths = [...new Bun.Glob("packs/*/v1.json").scanSync(".")].sort();
  const catalog: AuthoredLawCatalogEntry[] = [];
  for (const path of paths) {
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
        lawId: value.packId + ":" + law.id,
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

function verifyReviewDigests(fixture: AuthoredScientificFixture): void {
  for (const scenario of fixture.scenarios) {
    const digest = sha256(
      authoredScenarioReviewPayload(fixture, scenario.id),
    );
    if (scenario.review.finalDigest !== digest) {
      throw new Error(scenario.id + ": final review digest is stale");
    }
  }
}

async function rejectExternalHoldoutLeakage(
  holdout: AuthoredScientificFixture,
): Promise<void> {
  const authoredPaths = new Set([
    "fixtures/challenge/document-reasoning-development-v1.json",
    "fixtures/challenge/document-reasoning-holdout-v1.json",
  ]);
  const otherDocuments = new Set<string>();
  for await (const path of new Bun.Glob("fixtures/**/*.json").scan(".")) {
    if (authoredPaths.has(path)) continue;
    collectDocumentContent(
      JSON.parse(await readFile(path, "utf8")),
      otherDocuments,
    );
  }
  for (const scenario of holdout.scenarios) {
    for (const snapshot of scenario.snapshots) {
      for (const document of snapshot.documents) {
        if (otherDocuments.has(normalize(document.content))) {
          throw new Error(
            scenario.id + ": frozen document duplicates an existing fixture",
          );
        }
      }
    }
  }
}

function collectDocumentContent(
  value: unknown,
  output: Set<string>,
): void {
  if (Array.isArray(value)) {
    for (const item of value) collectDocumentContent(item, output);
    return;
  }
  if (typeof value !== "object" || value === null) return;
  const item = value as Record<string, unknown>;
  if (typeof item.content === "string") {
    output.add(normalize(item.content));
  }
  for (const child of Object.values(item)) {
    collectDocumentContent(child, output);
  }
}

function normalize(value: string): string {
  return value.toLowerCase().replaceAll(/\s+/gu, " ").trim();
}

function verifyAuthoredSplitIntegrity(
  development: AuthoredScientificFixture,
  holdout: AuthoredScientificFixture,
): { readonly maximumProse: number } {
  const comparisons = compareAuthoredIntegrityProfiles(
    development.scenarios.map(integrityProfile),
    holdout.scenarios.map(integrityProfile),
  );
  const suspicious = comparisons.filter(
    (comparison) =>
      comparison.proseSimilarity >= 0.5 ||
      (comparison.exactMath && comparison.proseSimilarity >= 0.25),
  );
  if (suspicious.length > 0) {
    const examples = suspicious
      .sort((left, right) => right.proseSimilarity - left.proseSimilarity)
      .slice(0, 5)
      .map(
        (comparison) =>
          comparison.developmentId +
          "/" +
          comparison.holdoutId +
          " (math=" +
          comparison.mathSimilarity.toFixed(3) +
          ", prose=" +
          comparison.proseSimilarity.toFixed(3) +
          ")",
      );
    throw new Error(
      "authored development/holdout lineage similarity requires review: " +
        examples.join(", "),
    );
  }
  return {
    maximumProse: Math.max(
      0,
      ...comparisons.map((comparison) => comparison.proseSimilarity),
    ),
  };
}

function integrityProfile(
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
        language: /\.md$/iu.test(document.path) ? "markdown" : "latex",
      })),
    });
    for (const document of snapshot.documents) {
      const syntax = service.getFile(document.fileId);
      if (!syntax) throw new Error(`${scenario.id}: missing wasmtex syntax`);
      if (syntax.diagnostics.length > 0) {
        throw new Error(
          `${scenario.id}/${document.fileId}: authored source has invalid TeX`,
        );
      }
      for (const fingerprint of authoredMathFingerprints(syntax)) {
        math.add(fingerprint);
      }
      for (const shingle of authoredProseShingles(document.content, syntax)) {
        prose.add(shingle);
      }
    }
  }
  return {
    id: scenario.id,
    mathFingerprints: [...math].sort(),
    proseShingles: [...prose].sort(),
  };
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}
