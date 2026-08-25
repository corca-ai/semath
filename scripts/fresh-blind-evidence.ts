import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { isAbsolute, relative, resolve } from "node:path";
import {
  LatexSyntaxService,
  type LatexFileSyntax,
  type LatexNotationNode,
  type LatexSyntaxRange,
} from "wasmtex/syntax";
import {
  authoredFixtureSealPayload,
  authoredMathFingerprints,
  authoredProseShingles,
  authoredScenarioFor,
  authoredScenarioReviewPayload,
  authoredSnapshotFor,
  freshBlindSealPayload,
  parseFreshBlindReleaseFixture,
  spentHoldoutProfile,
  validateFreshBlindRelease,
  type AuthoredIntegrityProfile,
  type AuthoredLawCatalogEntry,
  type AuthoredScientificScenario,
  type FreshBlindReleaseFixture,
  type FreshBlindSnapshotSyntaxFacts,
  type FreshBlindValidationSummary,
  type SpentHoldoutLineage,
} from "../packages/evaluation/src/index";

export interface LoadedFreshBlindEvidence {
  readonly path: string;
  readonly release: FreshBlindReleaseFixture;
  readonly summary: FreshBlindValidationSummary;
}

export interface FreshBlindReferenceManifest {
  readonly entries: readonly {
    readonly path: string;
    readonly sha256: string;
  }[];
  readonly sha256: string;
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
  const references = await referenceEvidence();
  const reviewDigests = Object.fromEntries(
    release.fixture.scenarios.map((scenario) => [
      scenario.id,
      sha256(authoredScenarioReviewPayload(release.fixture, scenario.id)),
    ]),
  );
  const summary = validateFreshBlindRelease(release, {
    authoredSealDigest: sha256(authoredFixtureSealPayload(release.fixture)),
    authoringSyntaxFacts: freshAuthoringSyntaxFacts(release),
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

export function freshAuthoringSyntaxFacts(
  release: FreshBlindReleaseFixture,
): readonly FreshBlindSnapshotSyntaxFacts[] {
  return freshAuthoringSyntaxFactsForSelections(
    release.fixture.probes.map((probe) => {
      const scenario = authoredScenarioFor(release.fixture, probe);
      return {
        scenarioId: scenario.id,
        snapshot: authoredSnapshotFor(scenario, probe),
      };
    }),
  );
}

export function freshAuthoringSyntaxFactsForSelections(
  selections: readonly {
    readonly scenarioId: string;
    readonly snapshot: AuthoredScientificScenario["snapshots"][number];
  }[],
): readonly FreshBlindSnapshotSyntaxFacts[] {
  const selected = new Map<string, FreshBlindSnapshotSyntaxFacts>();
  for (const { scenarioId, snapshot } of selections) {
    const key = `${scenarioId}\0${snapshot.id}`;
    if (selected.has(key)) continue;
    const service = new LatexSyntaxService();
    service.reset({
      documents: snapshot.documents.map((document) => ({
        ...document,
        documentVersion: 1,
        language: languageOf(document.path),
      })),
    });
    selected.set(key, {
      documents: snapshot.documents.map((document) => {
        const syntax = service.getFile(document.fileId);
        if (!syntax) throw new Error(`${scenarioId}: missing wasmtex syntax`);
        return {
          fileId: document.fileId,
          mathRootContentRanges: syntax.mathRoots.map((root) => ({
            ...root.contentRange,
          })),
          occurrences: neutralOccurrenceFacts(document.content, syntax),
        };
      }),
      scenarioId,
      snapshotId: snapshot.id,
    });
  }
  return [...selected.values()];
}

function neutralOccurrenceFacts(
  content: string,
  syntax: LatexFileSyntax,
): NonNullable<
  FreshBlindSnapshotSyntaxFacts["documents"][number]["occurrences"]
> {
  const structuralCommands = new Set([
    "dfrac",
    "frac",
    "sqrt",
    "tfrac",
  ]);
  const structuralOnlyCommands = new Set([
    "exists",
    "forall",
    "int",
    "lim",
    "sum",
  ]);
  const ignoredCommands = new Set([
    " ", ",", ":", ";", "!", "quad", "qquad", "enspace", "thinspace",
    "medspace", "thickspace", "negthinspace", "big", "Big", "bigl", "bigr",
    "Bigl", "Bigr", "displaystyle", "textstyle", "scriptstyle",
    "scriptscriptstyle", "rm", "label", "tag", "tag*", "notag", "nonumber",
    "mathord", "mathop", "mathbin", "mathrel", "mathopen", "mathclose",
    "mathinner",
  ]);
  const occurrences = syntax.nodes.flatMap((node, nodeId) => {
    if (ancestorHasKind(syntax.nodes, nodeId, "named-operator")) return [];
    const atom = node.kind === "token" && node.lexicalClass === "identifier" &&
        node.text
      ? {
          kind: "identifier" as const,
          selectionRange: node.ranges.full,
          surface: node.text,
        }
      : node.kind === "named-operator" && node.name
      ? {
          kind: "named-operator" as const,
          selectionRange: node.ranges.name ?? node.ranges.full,
          surface: node.name,
        }
      : node.kind === "command" && node.name &&
          !structuralCommands.has(node.name) && !ignoredCommands.has(node.name) &&
          (!structuralOnlyCommands.has(node.name) || node.state === "complete")
      ? {
          kind: "command" as const,
          selectionRange: node.ranges.command ?? node.ranges.full,
          surface: `\\${node.name}`,
        }
      : undefined;
    if (!atom) return [];
    const range = atom.kind === "command" && node.name &&
        structuralOnlyCommands.has(node.name)
      ? node.ranges.full
      : notationOccurrenceRange(syntax.nodes, atom.selectionRange);
    const surface = compositionalOccurrenceSurface(
      content,
      syntax.nodes,
      atom.selectionRange,
      range,
      atom.surface,
    );
    const applicationEndOffset = applicationEnd(
      syntax.nodes,
      atom.selectionRange,
      range,
    );
    return [{
      ...(applicationEndOffset === undefined ? {} : { applicationEndOffset }),
      kind: atom.kind,
      range: { ...range },
      selectionRange: { ...atom.selectionRange },
      surface,
    }];
  });
  const unique = new Map(
    occurrences.map((occurrence) => [JSON.stringify(occurrence), occurrence]),
  );
  return [...unique.values()].sort((left, right) =>
    left.selectionRange.startOffset - right.selectionRange.startOffset ||
    left.selectionRange.endOffset - right.selectionRange.endOffset ||
    left.range.startOffset - right.range.startOffset ||
    left.range.endOffset - right.range.endOffset ||
    compareOccurrenceText(left.surface, right.surface) ||
    compareOccurrenceText(left.kind, right.kind) ||
    (left.applicationEndOffset ?? -1) - (right.applicationEndOffset ?? -1)
  );
}

function compositionalOccurrenceSurface(
  content: string,
  nodes: readonly LatexNotationNode[],
  selection: LatexSyntaxRange,
  occurrence: LatexSyntaxRange,
  surface: string,
): string {
  const path = nodes
    .filter((node) => rangeContainsRange(node.ranges.full, selection))
    .sort((left, right) =>
      (right.ranges.full.endOffset - right.ranges.full.startOffset) -
      (left.ranges.full.endOffset - left.ranges.full.startOffset)
    );
  const hasCompositeComponent = path.some((node) => {
    if ((node.kind === "modifier" || node.kind === "style") && node.name) {
      return true;
    }
    if (node.kind !== "script") return false;
    if (node.name === "superscript") return true;
    if (node.name !== "subscript") return false;
    const base = node.children[0] === undefined
      ? ""
      : boundedNotationText(nodes, node.children[0], 0);
    const index = node.children[1] === undefined
      ? ""
      : boundedNotationText(nodes, node.children[1], 0);
    return base.length > 0 && index.length > 0;
  });
  if (hasCompositeComponent) {
    return content.slice(occurrence.startOffset, occurrence.endOffset);
  }
  return path.find((node) => node.kind === "named-operator")?.name ?? surface;
}

function boundedNotationText(
  nodes: readonly LatexNotationNode[],
  nodeId: number,
  depth: number,
): string {
  if (depth === 8) return "";
  const node = nodes[nodeId];
  if (!node) return "";
  if (node.text !== undefined) return node.text;
  if (node.children.length === 0) return node.name ?? "";
  return node.children
    .map((child) => boundedNotationText(nodes, child, depth + 1))
    .join("");
}

function notationOccurrenceRange(
  nodes: readonly LatexNotationNode[],
  selection: LatexSyntaxRange,
): LatexSyntaxRange {
  let widest: LatexNotationNode | undefined;
  for (const node of nodes) {
    if (!notationIdentityContains(nodes, node, selection, 0)) continue;
    const width = node.ranges.full.endOffset - node.ranges.full.startOffset;
    const widestWidth = widest === undefined
      ? -1
      : widest.ranges.full.endOffset - widest.ranges.full.startOffset;
    // Rust Iterator::max_by_key returns the last equal maximum.
    if (width >= widestWidth) widest = node;
  }
  return widest?.ranges.full ?? selection;
}

function compareOccurrenceText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function notationIdentityContains(
  nodes: readonly LatexNotationNode[],
  node: LatexNotationNode,
  selection: LatexSyntaxRange,
  depth: number,
): boolean {
  if (depth === 16) return false;
  const identity = node.kind === "named-operator"
    ? node.ranges.name
    : node.kind === "modifier" || node.kind === "script"
    ? node.ranges.nucleus ?? node.arguments?.find((argument) =>
      argument.role === "nucleus"
    )?.range
    : node.kind === "style"
    ? node.arguments?.find((argument) => argument.role === "body")?.range
    : undefined;
  if (!identity || !rangeContainsRange(identity, selection)) return false;
  const child = node.children
    .map((childId) => nodes[childId])
    .find((candidate) => candidate &&
      rangeContainsRange(identity, candidate.ranges.full) &&
      rangeContainsRange(candidate.ranges.full, selection)
    );
  return child === undefined ||
    identityDescendantContains(nodes, child, selection, depth + 1);
}

function identityDescendantContains(
  nodes: readonly LatexNotationNode[],
  node: LatexNotationNode,
  selection: LatexSyntaxRange,
  depth: number,
): boolean {
  if (
    node.kind === "named-operator" || node.kind === "modifier" ||
    node.kind === "script" || node.kind === "style"
  ) return notationIdentityContains(nodes, node, selection, depth);
  if (depth === 16) return false;
  const child = node.children
    .map((childId) => nodes[childId])
    .find((candidate) => candidate &&
      rangeContainsRange(candidate.ranges.full, selection)
    );
  return child === undefined ||
    identityDescendantContains(nodes, child, selection, depth + 1);
}

function applicationEnd(
  nodes: readonly LatexNotationNode[],
  selection: LatexSyntaxRange,
  occurrence: LatexSyntaxRange,
): number | undefined {
  const path = nodes
    .map((node, nodeId) => ({ node, nodeId }))
    .filter(({ node }) => rangeContainsRange(node.ranges.full, selection))
    .sort((left, right) =>
      (right.node.ranges.full.endOffset - right.node.ranges.full.startOffset) -
      (left.node.ranges.full.endOffset - left.node.ranges.full.startOffset)
    );
  for (const { node, nodeId } of path) {
    if (
      (node.kind !== "named-operator" && node.kind !== "token") ||
      node.ranges.full.startOffset < occurrence.startOffset ||
      node.ranges.full.endOffset > occurrence.endOffset
    ) continue;
    const sibling = nextMeaningfulSibling(nodes, nodeId);
    if (
      sibling && (sibling.kind === "delimiter" || sibling.kind === "group") &&
      sibling.state === "complete" &&
      sibling.ranges.full.startOffset < sibling.ranges.full.endOffset
    ) return sibling.ranges.full.endOffset;
  }
  return undefined;
}

function nextMeaningfulSibling(
  nodes: readonly LatexNotationNode[],
  nodeId: number,
): LatexNotationNode | undefined {
  const parentId = nodes[nodeId]?.parent;
  if (parentId === null || parentId === undefined) return undefined;
  const parent = nodes[parentId];
  const position = parent?.children.indexOf(nodeId) ?? -1;
  if (!parent || position < 0) return undefined;
  return parent.children.slice(position + 1)
    .map((siblingId) => nodes[siblingId])
    .find((sibling) =>
      sibling !== undefined &&
      (sibling.text === undefined || sibling.text.trim() !== "")
    );
}

function ancestorHasKind(
  nodes: readonly LatexNotationNode[],
  nodeId: number,
  kind: LatexNotationNode["kind"],
): boolean {
  for (let parent = nodes[nodeId]?.parent; parent !== null && parent !== undefined;) {
    const node = nodes[parent];
    if (!node) return false;
    if (node.kind === kind) return true;
    parent = node.parent;
  }
  return false;
}

function rangeContainsRange(
  container: LatexSyntaxRange,
  contained: LatexSyntaxRange,
): boolean {
  return container.startOffset <= contained.startOffset &&
    contained.endOffset <= container.endOffset;
}

export function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

export function freshSpentHoldoutLineage(
  release: FreshBlindReleaseFixture,
): SpentHoldoutLineage {
  return {
    batchId: release.fixture.batch.id,
    probeIds: release.fixture.probes.map((probe) => probe.id).sort(),
    profiles: release.fixture.scenarios
      .flatMap((scenario) => {
        const documents = scenario.snapshots.flatMap((snapshot) =>
          snapshot.documents.map((document) => ({
            document,
            id: `${scenario.id}/${snapshot.id}/${document.fileId}`,
          })),
        );
        return [
          spentHoldoutProfile(
            scenario.id,
            documents.map(({ document }) => sha256(document.content)),
            scenarioProfile(scenario),
            sha256,
          ),
          ...documents.map(({ document, id }) =>
            spentHoldoutProfile(
              id,
              [sha256(document.content)],
              documentProfile({
                content: document.content,
                id,
                path: document.path,
              }),
              sha256,
            ),
          ),
        ];
      })
      .sort((left, right) => left.id.localeCompare(right.id)),
    rawDigests: release.fixture.scenarios
      .map((scenario) => scenario.provenance.rawDigest)
      .sort(),
    releaseId: release.release.id,
    scenarioIds: release.fixture.scenarios.map((scenario) => scenario.id).sort(),
  };
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

async function referenceEvidence(): Promise<{
  readonly documents: readonly string[];
  readonly profiles: readonly AuthoredIntegrityProfile[];
}> {
  const documents = new Map<string, ReferenceDocument>();
  for (const fixturePath of approvedReferenceFixturePaths()) {
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

export const APPROVED_CHALLENGE_REFERENCES = [
  "fixtures/challenge/document-reasoning-development-v1.json",
  "fixtures/challenge/domain-routing-v1.json",
  "fixtures/challenge/equivalence-v1.json",
  "fixtures/challenge/math-authoring-oracle-source-v2.json",
  "fixtures/challenge/recognition-frontier-v1.json",
  "fixtures/challenge/recognition-v2.json",
  "fixtures/challenge/recognition-v3.json",
  "fixtures/challenge/semantic-continuity-v1.json",
] as const;

/** Historical holdout/fresh namespaces are never opened during commissioning.
 * Only explicitly public development evidence may participate in isolation. */
export function isApprovedReferenceFixturePath(path: string): boolean {
  const normalized = path.replaceAll("\\", "/").replace(/^\.\//u, "");
  return (
    APPROVED_CHALLENGE_REFERENCES.includes(
      normalized as (typeof APPROVED_CHALLENGE_REFERENCES)[number],
    ) ||
    /^(?:fixtures\/(?:corpus|development|foundation))\/.+\.json$/u.test(
      normalized,
    )
  );
}

export function approvedReferenceFixturePaths(): string[] {
  const publicDirectories = ["corpus", "development", "foundation"].flatMap(
    (directory) => [
      ...new Bun.Glob(`fixtures/${directory}/**/*.json`).scanSync("."),
    ],
  );
  return [...APPROVED_CHALLENGE_REFERENCES, ...publicDirectories]
    .filter(isApprovedReferenceFixturePath)
    .sort();
}

/** A sealed, path-and-byte inventory of only the public references that the
 * commissioning validator is allowed to open. */
export async function freshBlindReferenceManifest(): Promise<FreshBlindReferenceManifest> {
  const entries = await Promise.all(
    approvedReferenceFixturePaths().map(async (path) => ({
      path,
      sha256: sha256(await readFile(path)),
    })),
  );
  const bytes = `${JSON.stringify(entries)}\n`;
  return { entries, sha256: sha256(bytes) };
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
