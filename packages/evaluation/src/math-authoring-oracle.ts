import { createHash } from "node:crypto";
import { posix as path } from "node:path";
import type {
  MathAuthoringContext,
  MathInterpretationEvidenceInfo,
  MathInterpretationEvidenceReferenceInfo,
  MathInterpretationHypothesisInfo,
  MathInterpretationAnalysisLimitInfo,
  MathInterpretationRequirementInfo,
  LawConditionInfo,
  SourceRange,
  MathInterpretationPreCapSemanticKey,
} from "../../protocol/src/index";
import {
  MATH_INTERPRETATION_HYPOTHESIS_LIMIT,
  canonicalMathInterpretationPreCapPayload,
} from "../../protocol/src/index";
import {
  compareMathAuthoringContext,
  mathAuthoringContextSafetyFailures,
  parseObservedMathAuthoringContext,
  projectMathAuthoringContext,
  type MathAuthoringContextFailure,
  type StableMathAuthoringContext,
} from "./math-authoring-development";

export type MathAuthoringOracleFacet =
  | "approximation" | "cap" | "claim-evidence" | "clean-incremental"
  | "conditions" | "conventional-candidates" | "cross-document"
  | "equation-links" | "generated" | "interpretations" | "lifecycle"
  | "notation" | "requirements" | "retraction-transition";

export interface MathAuthoringOracleSource {
  readonly cases: readonly MathAuthoringOracleSourceCase[];
  readonly fixtureId: string;
  readonly pairs: readonly {
    readonly id: string;
    readonly latexCaseId: string;
    readonly markdownCaseId: string;
  }[];
  readonly schemaVersion: 2;
}

export interface MathAuthoringOracleSourceCase {
  readonly id: string;
  readonly language: "latex" | "markdown";
  readonly namedNeedles: readonly {
    readonly fileId: string;
    readonly id: string;
    readonly needle: string;
    readonly occurrence?: number;
    readonly parentAnchor?: string;
    readonly snapshotId: string;
    readonly within?: { readonly anchor: string; readonly needle: string; readonly occurrence?: number };
  }[];
  readonly pairId: string;
  readonly selections: readonly {
    readonly anchor: string;
    readonly id: string;
    readonly snapshotId: string;
  }[];
  readonly snapshots: readonly {
    readonly dependencies: readonly {
      readonly fromFileId: string;
      readonly sourceAnchor: string;
      readonly toFileId: string;
    }[];
    readonly documents: readonly {
      readonly content: string;
      readonly documentVersion: number;
      readonly fileId: string;
      readonly path: string;
    }[];
    readonly id: string;
    readonly mainFileId: string;
  }[];
}

export interface EvidenceConstraint {
  readonly anchors: readonly string[];
  readonly generation: "authored" | "generated";
  readonly kind: string;
  readonly lifecycle: "current" | "retracted";
  readonly provenance?: MathInterpretationEvidenceInfo["provenance"];
  readonly role?: MathInterpretationEvidenceInfo["role"];
  readonly ruleId?: string;
  readonly strength: string;
}

export interface HypothesisSelector {
  readonly formulaAnchor: string;
  readonly kind: MathInterpretationHypothesisInfo["kind"];
  readonly label: string;
  readonly relationId?: string;
}

export interface RequiredHypothesisConstraint {
  readonly bindings: readonly {
    readonly parameter: string;
    readonly symbol: string;
  }[];
  readonly conditions: readonly {
    readonly conditionId: string;
    readonly label: string;
    readonly status: LawConditionInfo["status"];
  }[];
  readonly dependentFacets: readonly MathAuthoringOracleFacet[];
  readonly evidence: readonly string[];
  readonly id: string;
  readonly releaseRequired: boolean;
  readonly selector: HypothesisSelector;
  readonly supportAllowed: readonly MathInterpretationHypothesisInfo["support"][];
}

export interface RequirementConstraint {
  readonly conditionLabel?: string;
  readonly kind: MathInterpretationRequirementInfo["kind"];
  readonly parameter?: string;
  readonly symbol?: string;
}

export interface MathAuthoringCaseConstraint {
  readonly advisory: {
    readonly allowedExtras: {
      readonly anchorAllowlist: readonly string[];
      readonly kinds: readonly MathInterpretationHypothesisInfo["kind"][];
      readonly maxCount: number;
      readonly provenances: readonly MathInterpretationEvidenceInfo["provenance"][];
      readonly supportAllowed: readonly MathInterpretationHypothesisInfo["support"][];
    };
    readonly coverageGoals: readonly {
      readonly facet: MathAuthoringOracleFacet;
      readonly rationale: string;
    }[];
    readonly knownMisses: readonly { readonly facet: MathAuthoringOracleFacet; readonly rationale: string }[];
    readonly relativeOrder: readonly { readonly after: string; readonly before: string }[];
    readonly requiredHypotheses: readonly RequiredHypothesisConstraint[];
    readonly requiredMissingDiscriminators: readonly RequirementConstraint[];
    readonly requiredRequirements: readonly RequirementConstraint[];
  };
  readonly cap?: {
    readonly correctHypothesisId: string;
    readonly exposedExact: number;
    readonly preCapRequiredHypotheses: readonly {
      readonly formulaGeneration: "authored" | "generated";
      readonly formulaLifecycle: "current" | "retracted";
      readonly requiredHypothesisId: string;
    }[];
    readonly requiredLimitKinds: readonly MathInterpretationAnalysisLimitInfo["kind"][];
  };
  readonly facets: readonly MathAuthoringOracleFacet[];
  readonly id: string;
  readonly selectionId: string;
  readonly safety: {
    readonly approximation?: { readonly exactness: "approximate"; readonly relationAnchor: string };
    readonly claims: readonly {
      readonly anchor: string;
      readonly modality: string;
      readonly polarity: string;
      readonly strengthCeiling: string;
    }[];
    readonly disposition: MathAuthoringContext["disposition"];
    readonly equationLinks: readonly {
      readonly kind: string;
      readonly sourceAnchor: string;
      readonly targetAnchor: string;
    }[];
    readonly forbiddenAuthority: readonly HypothesisSelector[];
    readonly formulaAnchor: string;
    readonly interpretationsTruncated: boolean;
    readonly lifecycle: MathAuthoringContext["lifecycle"];
    readonly limits: readonly { readonly evidence: readonly string[]; readonly kind: string }[];
    readonly noUnexpectedAuthority: boolean;
    readonly noUnexpectedContradictions: boolean;
    readonly generatedSubnodes: readonly {
      readonly anchor: string;
      readonly evidence: readonly string[];
    }[];
    readonly notation: readonly {
      readonly anchor: string;
      readonly sourceNotation: string;
    }[];
    readonly requiredAuthority: readonly string[];
    readonly requiredContradictions: readonly string[];
    readonly truncated: boolean;
  };
  readonly sourceCaseId: string;
  readonly transition?: {
    readonly after: {
      readonly disposition: MathAuthoringContext["disposition"];
      readonly forbiddenAnchors: readonly string[];
      readonly forbiddenAuthority: readonly HypothesisSelector[];
      readonly formulaAnchor: string;
      readonly lifecycle: MathAuthoringContext["lifecycle"];
      readonly requiredMissingDiscriminators: readonly RequirementConstraint[];
      readonly snapshotId: string;
    };
    readonly before: {
      readonly disposition: MathAuthoringContext["disposition"];
      readonly formulaAnchor: string;
      readonly lifecycle: MathAuthoringContext["lifecycle"];
      readonly requiredAnchors: readonly string[];
      readonly requiredAuthority: readonly string[];
      readonly snapshotId: string;
    };
    readonly cleanIncremental: true;
    readonly removed?: {
      readonly context: "absent";
      readonly selectionAnchor: string;
      readonly snapshotId: string;
    };
  };
}

export interface MathAuthoringOracle {
  readonly cases: readonly MathAuthoringCaseConstraint[];
  readonly evidence: Readonly<Record<string, EvidenceConstraint>>;
  readonly pairs: readonly {
    readonly compare: {
      readonly authority: "exact";
      readonly hypotheses: readonly ("kind" | "label" | "relationId" | "formulaAnchor" | "support" | "bindings" | "conditions" | "evidence")[];
      readonly lifecycle: "exact";
      readonly limits: "exact";
      readonly ordering: "required-relative";
    };
    readonly id: string;
    readonly markdownCaseId: string;
    readonly texCaseId: string;
  }[];
  readonly review: {
    readonly attestationDigest: string;
    readonly author: string;
    readonly digest: string;
    readonly reviewFixture: string;
    readonly reviewedAt: string;
    readonly reviewer: string;
  };
  readonly schemaVersion: 2;
  readonly sourceFixture: string;
  readonly sourceFixtureId: string;
  readonly sourceSha256: string;
}

export interface MathAuthoringOracleReviewAttestation {
  readonly oracleConstraintSha256: string;
  readonly reviewedAt: string;
  readonly reviewer: string;
  readonly schemaVersion: 2;
  readonly sourceFixture: string;
  readonly sourceFixtureId: string;
  readonly sourceSha256: string;
  readonly verdict: "approved";
}

export interface MathAuthoringExpectedObservation {
  readonly caseId: string;
  readonly context: "absent" | "present";
  readonly mode: "clean" | "incremental";
  readonly selection: ResolvedNamedAnchor;
  readonly selectionAnchorId: string;
  readonly snapshotId: string;
  readonly sourceCaseId: string;
}

export interface ResolvedNamedAnchor {
  readonly caseId: string;
  readonly documentVersion: number;
  readonly fileId: string;
  readonly location: { readonly fileId: string; readonly path: string; readonly range: SourceRange };
  readonly logicalId: string;
  readonly snapshotId: string;
}

export interface CompiledMathAuthoringOracle {
  readonly anchors: Readonly<Record<string, ResolvedNamedAnchor>>;
  readonly capExpectations: Readonly<Record<string, {
    readonly candidateCountBeforeCap: number;
    readonly preCapSemanticKeyDigest: string;
    readonly semanticKeys: readonly MathInterpretationPreCapSemanticKey[];
  }>>;
  readonly oracle: MathAuthoringOracle;
  readonly source: MathAuthoringOracleSource;
}

export interface MathAuthoringOracleObservation {
  readonly authoringContext?: MathAuthoringContext;
  readonly caseId: string;
  readonly mode: "clean" | "incremental";
  readonly selection: {
    readonly documentVersion: number;
    readonly location: { readonly fileId: string; readonly path: string; readonly range: SourceRange };
  };
  readonly snapshotId: string;
}

export interface MathAuthoringOracleReport {
  readonly advisoryFindings: readonly string[];
  readonly diagnostic: { readonly artifactId: string; readonly sha256: string };
  readonly pairFailures: readonly string[];
  readonly safetyFailures: readonly string[];
  readonly suppressedFacets: readonly MathAuthoringOracleFacet[];
  readonly transitionFailures: readonly string[];
}

const facets: readonly MathAuthoringOracleFacet[] = [
  "approximation", "cap", "claim-evidence", "clean-incremental", "conditions",
  "conventional-candidates", "cross-document", "equation-links", "generated",
  "interpretations", "lifecycle", "notation", "requirements", "retraction-transition",
];
const hypothesisKinds = ["source-meaning", "typed-law", "scoped-domain", "structural-alternative", "reviewed-convention"] as const;
const supportTiers = ["explicit", "derived", "supported", "tentative", "contradicted"] as const;
const provenances = ["explicit-declaration", "typed-structure", "natural-language-extraction", "domain-context", "reviewed-convention", "derived-evidence"] as const;
const canonicalSourceFixture = "fixtures/challenge/math-authoring-oracle-source-v2.json";
const canonicalSourceFixtureId = "semath-math-authoring-public-source-v2";
const canonicalReviewFixture = "fixtures/challenge/math-authoring-oracle-review-v2.json";

export function parseMathAuthoringOracleSource(value: unknown): MathAuthoringOracleSource {
  const root = object(value, "source", ["cases", "fixtureId", "pairs", "schemaVersion"]);
  if (root.schemaVersion !== 2) throw new Error("source.schemaVersion: expected 2");
  const cases = array(root.cases, "source.cases").map((value, index) => {
    const path = `source.cases[${index}]`;
    const item = object(value, path, ["id", "language", "namedNeedles", "pairId", "selections", "snapshots"]);
    const rawSnapshots = array(item.snapshots, `${path}.snapshots`).map((value, snapshotIndex) => {
      const snapshotPath = `${path}.snapshots[${snapshotIndex}]`;
      const snapshot = object(value, snapshotPath, ["dependencies", "documents", "id", "mainFileId"]);
      const documents = array(snapshot.documents, `${snapshotPath}.documents`).map((value, documentIndex) => {
        const documentPath = `${snapshotPath}.documents[${documentIndex}]`;
        const document = object(value, documentPath, ["content", "fileId", "path"]);
        return { content: text(document.content, `${documentPath}.content`), fileId: text(document.fileId, `${documentPath}.fileId`), path: text(document.path, `${documentPath}.path`) };
      });
      unique(documents.map((document) => document.fileId), `${snapshotPath}.documents.fileId`);
      for (const document of documents) normalizedSourcePath(document.path, `${snapshotPath}.documents.path`);
      unique(documents.map((document) => document.path), `${snapshotPath}.documents.path`);
      const dependencies = array(snapshot.dependencies, `${snapshotPath}.dependencies`).map((value, dependencyIndex) => {
        const dependencyPath = `${snapshotPath}.dependencies[${dependencyIndex}]`;
        const dependency = object(value, dependencyPath, ["fromFileId", "sourceAnchor", "toFileId"]);
        return {
          fromFileId: text(dependency.fromFileId, `${dependencyPath}.fromFileId`),
          sourceAnchor: text(dependency.sourceAnchor, `${dependencyPath}.sourceAnchor`),
          toFileId: text(dependency.toFileId, `${dependencyPath}.toFileId`),
        };
      });
      const mainFileId = text(snapshot.mainFileId, `${snapshotPath}.mainFileId`);
      const fileIds = new Set(documents.map((document) => document.fileId));
      if (!fileIds.has(mainFileId)) throw new Error(`${snapshotPath}.mainFileId: unknown document`);
      for (const dependency of dependencies) {
        if (!fileIds.has(dependency.fromFileId) || !fileIds.has(dependency.toFileId)) throw new Error(`${snapshotPath}.dependencies: unknown fileId`);
        if (dependency.fromFileId === dependency.toFileId) throw new Error(`${snapshotPath}.dependencies: self dependency is forbidden`);
      }
      unique(dependencies.map((dependency) => `${dependency.fromFileId}\u0000${dependency.toFileId}\u0000${dependency.sourceAnchor}`), `${snapshotPath}.dependencies`);
      assertAcyclicDependencies(fileIds, dependencies, `${snapshotPath}.dependencies`);
      assertReachableDependencies(mainFileId, fileIds, dependencies, `${snapshotPath}.dependencies`);
      return { dependencies, documents, id: text(snapshot.id, `${snapshotPath}.id`), mainFileId };
    });
    unique(rawSnapshots.map((snapshot) => snapshot.id), `${path}.snapshots.id`);
    const versions = new Map<string, { content: string; path: string; version: number }>();
    const snapshots = rawSnapshots.map((snapshot) => ({
      ...snapshot,
      documents: snapshot.documents.map((document) => {
        const prior = versions.get(document.fileId);
        const documentVersion = prior === undefined ? 1 :
          prior.content === document.content && prior.path === document.path
            ? prior.version
            : prior.version + 1;
        versions.set(document.fileId, {
          content: document.content,
          path: document.path,
          version: documentVersion,
        });
        return { ...document, documentVersion };
      }),
    }));
    const namedNeedles = array(item.namedNeedles, `${path}.namedNeedles`).map((value, needleIndex) => {
      const needlePath = `${path}.namedNeedles[${needleIndex}]`;
      const needle = object(value, needlePath, ["fileId", "id", "needle", "snapshotId"], ["occurrence", "parentAnchor", "within"]);
      const within = needle.within === undefined ? undefined : object(needle.within, `${needlePath}.within`, ["anchor", "needle"], ["occurrence"]);
      if (needle.parentAnchor !== undefined && within) throw new Error(`${needlePath}: parentAnchor and within are mutually exclusive`);
      return { fileId: text(needle.fileId, `${needlePath}.fileId`), id: text(needle.id, `${needlePath}.id`), needle: text(needle.needle, `${needlePath}.needle`), ...(needle.occurrence === undefined ? {} : { occurrence: positive(needle.occurrence, `${needlePath}.occurrence`) }), ...(needle.parentAnchor === undefined ? {} : { parentAnchor: text(needle.parentAnchor, `${needlePath}.parentAnchor`) }), snapshotId: text(needle.snapshotId, `${needlePath}.snapshotId`), ...(within ? { within: { anchor: text(within.anchor, `${needlePath}.within.anchor`), needle: text(within.needle, `${needlePath}.within.needle`), ...(within.occurrence === undefined ? {} : { occurrence: positive(within.occurrence, `${needlePath}.within.occurrence`) }) } } : {}) };
    });
    unique(namedNeedles.map((needle) => needle.id), `${path}.namedNeedles.id`);
    for (const snapshot of snapshots) for (const dependency of snapshot.dependencies) {
      const anchor = namedNeedles.find((needle) => needle.id === dependency.sourceAnchor);
      if (!anchor || anchor.snapshotId !== snapshot.id || anchor.fileId !== dependency.fromFileId) {
        throw new Error(`${path}.snapshots.${snapshot.id}.dependencies: sourceAnchor must belong to the from-file snapshot`);
      }
    }
    const selections = array(item.selections, `${path}.selections`).map((value, selectionIndex) => { const selectionPath = `${path}.selections[${selectionIndex}]`; const selection = object(value, selectionPath, ["anchor", "id", "snapshotId"]); return { anchor: text(selection.anchor, `${selectionPath}.anchor`), id: text(selection.id, `${selectionPath}.id`), snapshotId: text(selection.snapshotId, `${selectionPath}.snapshotId`) }; });
    unique(selections.map((selection) => selection.id), `${path}.selections.id`);
    for (const selection of selections) {
      const anchor = namedNeedles.find((needle) => needle.id === selection.anchor);
      if (!anchor || anchor.snapshotId !== selection.snapshotId) throw new Error(`${path}.selections.${selection.id}: unknown anchor or snapshot mismatch`);
    }
    return { id: text(item.id, `${path}.id`), language: choice(item.language, ["latex", "markdown"], `${path}.language`), namedNeedles, pairId: text(item.pairId, `${path}.pairId`), selections, snapshots };
  });
  unique(cases.map((item) => item.id), "source.cases.id");
  if (cases.length !== 20) throw new Error("source.cases: expected 20 cases");
  const pairs = array(root.pairs, "source.pairs").map((value, index) => { const path = `source.pairs[${index}]`; const pair = object(value, path, ["id", "latexCaseId", "markdownCaseId"]); return { id: text(pair.id, `${path}.id`), latexCaseId: text(pair.latexCaseId, `${path}.latexCaseId`), markdownCaseId: text(pair.markdownCaseId, `${path}.markdownCaseId`) }; });
  unique(pairs.map((pair) => pair.id), "source.pairs.id");
  if (pairs.length !== 10) throw new Error("source.pairs: expected 10 pairs");
  const byId = new Map(cases.map((item) => [item.id, item]));
  for (const pair of pairs) {
    const latex = byId.get(pair.latexCaseId);
    const markdown = byId.get(pair.markdownCaseId);
    if (latex?.language !== "latex" || markdown?.language !== "markdown" || latex.pairId !== pair.id || markdown.pairId !== pair.id) throw new Error(`source pair ${pair.id}: invalid language or pair identity`);
    if (stableJson(sourceDependencyProjection(latex)) !== stableJson(sourceDependencyProjection(markdown))) {
      throw new Error(`source pair ${pair.id}: dependency topology mismatch`);
    }
  }
  const pairedIds = pairs.flatMap((pair) => [pair.latexCaseId, pair.markdownCaseId]);
  unique(pairedIds, "source.pairs.caseId");
  if (pairedIds.length !== cases.length) throw new Error("source.pairs: every case must belong to one pair");
  for (const item of cases) {
    const extension = item.language === "latex" ? ".tex" : ".md";
    if (item.snapshots.some((snapshot) =>
      snapshot.documents.some((document) => !document.path.endsWith(extension))
    )) throw new Error(`${item.id}: document path does not match declared language`);
  }
  return { cases, fixtureId: text(root.fixtureId, "source.fixtureId"), pairs, schemaVersion: 2 };
}

export function parseMathAuthoringOracle(value: unknown): MathAuthoringOracle {
  const root = object(value, "oracle", ["cases", "evidence", "pairs", "review", "schemaVersion", "sourceFixture", "sourceFixtureId", "sourceSha256"]);
  if (root.schemaVersion !== 2) throw new Error("oracle.schemaVersion: expected 2");
  const evidence = parseMap(root.evidence, "oracle.evidence", parseEvidenceConstraint);
  const cases = array(root.cases, "oracle.cases").map(parseCaseConstraint);
  if (cases.length !== 20) throw new Error("oracle.cases: expected 20 reviewed cases");
  unique(cases.map((item) => item.id), "oracle.cases.id");
  unique(cases.map((item) => item.sourceCaseId), "oracle.cases.sourceCaseId");
  const pairs = array(root.pairs, "oracle.pairs").map((value, index) => {
    const path = `oracle.pairs[${index}]`;
    const item = object(value, path, ["compare", "id", "markdownCaseId", "texCaseId"]);
    const compare = object(item.compare, `${path}.compare`, ["authority", "hypotheses", "lifecycle", "limits", "ordering"]);
    const hypothesisFields = array(compare.hypotheses, `${path}.compare.hypotheses`).map((field, fieldIndex) => choice(field, ["kind", "label", "relationId", "formulaAnchor", "support", "bindings", "conditions", "evidence"], `${path}.compare.hypotheses[${fieldIndex}]`));
    unique(hypothesisFields, `${path}.compare.hypotheses`);
    if (stableJson([...hypothesisFields].sort()) !== stableJson(["bindings", "conditions", "evidence", "formulaAnchor", "kind", "label", "relationId", "support"])) throw new Error(`${path}.compare.hypotheses: all semantic fields are required`);
    return {
      compare: {
        authority: choice(compare.authority, ["exact"], `${path}.compare.authority`),
        hypotheses: hypothesisFields,
        lifecycle: choice(compare.lifecycle, ["exact"], `${path}.compare.lifecycle`),
        limits: choice(compare.limits, ["exact"], `${path}.compare.limits`),
        ordering: choice(compare.ordering, ["required-relative"], `${path}.compare.ordering`),
      },
      id: text(item.id, `${path}.id`), markdownCaseId: text(item.markdownCaseId, `${path}.markdownCaseId`), texCaseId: text(item.texCaseId, `${path}.texCaseId`),
    };
  });
  unique(pairs.map((item) => item.id), "oracle.pairs.id");
  if (pairs.length !== 10) throw new Error("oracle.pairs: expected 10 same-meaning pairs");
  const pairedIds = pairs.flatMap((pair) => [pair.texCaseId, pair.markdownCaseId]);
  unique(pairedIds, "oracle.pairs.caseId");
  if (pairedIds.length !== cases.length) throw new Error("oracle.pairs: every case must belong to one pair");
  const review = object(root.review, "oracle.review", ["attestationDigest", "author", "digest", "reviewFixture", "reviewedAt", "reviewer"]);
  const author = reviewIdentity(review.author, "oracle.review.author");
  const reviewer = text(review.reviewer, "oracle.review.reviewer");
  reviewIdentity(reviewer, "oracle.review.reviewer");
  if (canonicalReviewIdentity(reviewer) === canonicalReviewIdentity(author)) {
    throw new Error("oracle.review.reviewer: reviewer must be independent from author");
  }
  const attestationDigest = sha256Text(review.attestationDigest, "oracle.review.attestationDigest");
  const digest = text(review.digest, "oracle.review.digest");
  if (!/^[0-9a-f]{64}$/u.test(digest)) throw new Error("oracle.review.digest: expected sha256");
  const reviewedAt = calendarDate(review.reviewedAt, "oracle.review.reviewedAt");
  return {
    cases,
    evidence,
    pairs,
    review: { attestationDigest, author, digest, reviewFixture: text(review.reviewFixture, "oracle.review.reviewFixture"), reviewedAt, reviewer },
    schemaVersion: 2,
    sourceFixture: text(root.sourceFixture, "oracle.sourceFixture"),
    sourceFixtureId: text(root.sourceFixtureId, "oracle.sourceFixtureId"),
    sourceSha256: sha256Text(root.sourceSha256, "oracle.sourceSha256"),
  };
}

export function compileMathAuthoringOracle(
  sourceValue: unknown,
  oracleValue: unknown,
  attestationValue: unknown,
): CompiledMathAuthoringOracle {
  const source = parseMathAuthoringOracleSource(sourceValue);
  const oracle = parseMathAuthoringOracle(oracleValue);
  const prettyBytes = Buffer.byteLength(JSON.stringify(oracleValue, null, 2), "utf8");
  if (prettyBytes > 150 * 1024) throw new Error(`oracle exceeds 150 KiB reviewability guard: ${prettyBytes}`);
  rejectCanonicalWireObjects(oracleValue, "oracle");
  const sourceSha256 = mathAuthoringOracleSourceDigest(source);
  if (oracle.sourceFixture !== canonicalSourceFixture ||
    oracle.sourceFixtureId !== canonicalSourceFixtureId ||
    source.fixtureId !== canonicalSourceFixtureId ||
    oracle.sourceSha256 !== sourceSha256) {
    throw new Error("oracle source binding: canonical path, fixture identity, or digest mismatch");
  }
  validateReviewAttestation(oracle, attestationValue);
  const sourceById = new Map(source.cases.map((item) => [item.id, item]));
  const caseById = new Map(oracle.cases.map((item) => [item.id, item]));
  const anchors: Record<string, ResolvedNamedAnchor> = {};
  const capExpectations: Record<string, CompiledMathAuthoringOracle["capExpectations"][string]> = {};
  const resolving = new Set<string>();
  const specs = new Map<string, {
    readonly sourceCase: MathAuthoringOracleSourceCase;
    readonly spec: MathAuthoringOracleSourceCase["namedNeedles"][number];
  }>(source.cases.flatMap((sourceCase) =>
    sourceCase.namedNeedles.map((spec) => [`${sourceCase.id}:${spec.id}`, { sourceCase, spec }] as const)
  ));
  const resolve = (key: string): ResolvedNamedAnchor => {
    if (anchors[key]) return anchors[key];
    const found = specs.get(key);
    if (!found) throw new Error(`source anchor ${key}: unknown`);
    const { sourceCase, spec } = found;
    if (resolving.has(key)) throw new Error(`source anchor ${key}: cyclic within anchor`);
    resolving.add(key);
    const snapshot = sourceCase.snapshots.find((item) => item.id === spec.snapshotId);
    const document = snapshot?.documents.find((item) => item.fileId === spec.fileId);
    if (!snapshot || !document) throw new Error(`source anchor ${key}: unknown snapshot or file`);
    let containerStart = 0;
    let containerEnd = document.content.length;
    if (spec.parentAnchor || spec.within) {
      const outerId = spec.parentAnchor ?? requiredValue(spec.within, `source anchor ${key}: within missing`).anchor;
      const outer = resolve(`${sourceCase.id}:${outerId}`);
      if (outer.snapshotId !== spec.snapshotId || outer.fileId !== spec.fileId) throw new Error(`source anchor ${key}.within: must resolve in the same document snapshot`);
      if (spec.within) {
        const nested = occurrenceRange(document.content, spec.within.needle, spec.within.occurrence, `source anchor ${key}.within`, outer.location.range);
        containerStart = nested.startOffset;
        containerEnd = nested.endOffset;
      } else {
        containerStart = outer.location.range.startOffset;
        containerEnd = outer.location.range.endOffset;
      }
    }
    const range = occurrenceRange(document.content, spec.needle, spec.occurrence, `source anchor ${key}`, { startOffset: containerStart, endOffset: containerEnd });
    const resolved = { caseId: sourceCase.id, documentVersion: document.documentVersion, fileId: spec.fileId, location: { fileId: spec.fileId, path: document.path, range }, logicalId: `${sourceCase.pairId}:${spec.id}`, snapshotId: spec.snapshotId };
    anchors[key] = resolved;
    resolving.delete(key);
    return resolved;
  };
  [...specs.keys()].forEach(resolve);
  for (const item of oracle.cases) {
    const sourceCase = sourceById.get(item.sourceCaseId);
    if (!sourceCase) throw new Error(`${item.id}: unknown sourceCaseId`);
    const primarySelection = sourceCase.selections.find((selection) => selection.id === item.selectionId);
    if (!primarySelection) throw new Error(`${item.id}: unknown selectionId`);
    const primaryAnchorId = `${sourceCase.id}:${primarySelection.anchor}`;
    if (!item.transition) {
      assertFormulaContainsSelection(
        item.id,
        sourceCase,
        anchors[item.safety.formulaAnchor],
        anchors[primaryAnchorId],
        "primary",
      );
    } else {
      if (item.safety.formulaAnchor !== item.transition.before.formulaAnchor || primarySelection.snapshotId !== item.transition.before.snapshotId) {
        throw new Error(`${item.id}: transition primary selection must belong to the before formula contract`);
      }
      assertFormulaContainsSelection(
        item.id,
        sourceCase,
        anchors[item.transition.before.formulaAnchor],
        anchors[primaryAnchorId],
        "transition before",
      );
      const snapshotIndex = (snapshotId: string): number => sourceCase.snapshots.findIndex((snapshot) => snapshot.id === snapshotId);
      const beforeIndex = snapshotIndex(item.transition.before.snapshotId);
      const afterIndex = snapshotIndex(item.transition.after.snapshotId);
      const removedIndex = item.transition.removed ? snapshotIndex(item.transition.removed.snapshotId) : undefined;
      if (beforeIndex < 0 || afterIndex <= beforeIndex || (removedIndex !== undefined && removedIndex <= afterIndex)) {
        throw new Error(`${item.id}: transition snapshots must be chronological before < after < removed`);
      }
      for (const [phase, contract] of [
        ["before", item.transition.before],
        ["after", item.transition.after],
      ] as const) {
        const selections = sourceCase.selections.filter((candidate) =>
          candidate.snapshotId === contract.snapshotId
        );
        if (selections.length !== 1) throw new Error(`${item.id}: transition snapshot ${contract.snapshotId} requires exactly one explicit source selection`);
        const selection = selections[0]!;
        assertFormulaContainsSelection(
          item.id,
          sourceCase,
          anchors[contract.formulaAnchor],
          anchors[`${sourceCase.id}:${selection.anchor}`],
          `transition ${phase}`,
        );
      }
      if (item.transition.removed) {
        const anchorId = item.transition.removed.selectionAnchor.slice(`${sourceCase.id}:`.length);
        if (!sourceCase.selections.some((selection) =>
          selection.snapshotId === item.transition!.removed!.snapshotId && selection.anchor === anchorId
        )) throw new Error(`${item.id}: transition snapshot ${item.transition.removed.snapshotId} requires an explicit source selection`);
      }
    }
    validateConstraintReferences(item, oracle, anchors);
    if (item.cap) {
      const semanticKeys = item.cap.preCapRequiredHypotheses.map((expected) =>
        compilePreCapSemanticKey(item, expected, oracle, anchors)
      );
      const payload = canonicalMathInterpretationPreCapPayload(semanticKeys);
      const canonicalKeys = JSON.parse(payload) as unknown;
      if (!Array.isArray(canonicalKeys)) throw new Error(`${item.id}: protocol pre-cap canonical payload is not an array`);
      if (canonicalKeys.length !== semanticKeys.length || canonicalKeys.length <= MATH_INTERPRETATION_HYPOTHESIS_LIMIT) {
        throw new Error(`${item.id}: cap requires more than ${MATH_INTERPRETATION_HYPOTHESIS_LIMIT} distinct reviewed semantic keys`);
      }
      if (item.cap.exposedExact !== MATH_INTERPRETATION_HYPOTHESIS_LIMIT || item.cap.exposedExact >= canonicalKeys.length) {
        throw new Error(`${item.id}: cap exposed count must equal policy max and omit a reviewed alternative`);
      }
      capExpectations[item.id] = {
        candidateCountBeforeCap: canonicalKeys.length,
        preCapSemanticKeyDigest: sha256(payload),
        semanticKeys,
      };
    }
  }
  for (const pair of oracle.pairs) {
    const tex = caseById.get(pair.texCaseId);
    const md = caseById.get(pair.markdownCaseId);
    const texSource = tex && sourceById.get(tex.sourceCaseId);
    const mdSource = md && sourceById.get(md.sourceCaseId);
    if (!tex || !md || texSource?.language !== "latex" || mdSource?.language !== "markdown" || texSource.pairId !== pair.id || mdSource.pairId !== pair.id) throw new Error(`${pair.id}: pair must reference one same-meaning LaTeX and Markdown source pair`);
    const texHypothesisIds = tex.advisory.requiredHypotheses.map((item) => item.id).sort();
    const mdHypothesisIds = md.advisory.requiredHypotheses.map((item) => item.id).sort();
    if (stableJson(texHypothesisIds) !== stableJson(mdHypothesisIds) ||
      stableJson(tex.advisory.relativeOrder) !== stableJson(md.advisory.relativeOrder)) {
      throw new Error(`${pair.id}: paired constraints must share hypothesis identities and relative order`);
    }
    if (stableJson(sourceRelativeRequiredHypothesesProjection(tex, oracle, anchors)) !==
      stableJson(sourceRelativeRequiredHypothesesProjection(md, oracle, anchors))) {
      throw new Error(`${pair.id}: paired required hypotheses must share source-relative exact contracts`);
    }
    if (stableJson(sourceRelativePairSafetyProjection(tex, oracle, anchors)) !==
      stableJson(sourceRelativePairSafetyProjection(md, oracle, anchors))) {
      throw new Error(`${pair.id}: paired safety envelopes must be source-relative compatible`);
    }
    if ((tex.cap === undefined) !== (md.cap === undefined)) throw new Error(`${pair.id}: cap coverage must exist on both formats`);
    if (tex.cap && md.cap && stableJson(sourceRelativeCapProjection(tex, oracle, anchors)) !== stableJson(sourceRelativeCapProjection(md, oracle, anchors))) {
      throw new Error(`${pair.id}: paired cap constraints must share source-relative semantic identities`);
    }
  }
  const digest = mathAuthoringOracleReviewDigest(source, oracle);
  if (digest !== oracle.review.digest) throw new Error("oracle.review.digest: stale source+oracle review digest");
  return { anchors, capExpectations, oracle, source };
}

export function mathAuthoringOracleReviewDigest(
  source: MathAuthoringOracleSource,
  oracle: MathAuthoringOracle,
): string {
  const { digest: _digest, ...review } = oracle.review;
  return sha256(stableJson({ oracle: { ...oracle, review }, source }));
}

export function mathAuthoringOracleConstraintDigest(
  oracle: MathAuthoringOracle,
): string {
  const { review: _review, ...constraint } = oracle;
  return sha256(stableJson(constraint));
}

export function parseMathAuthoringOracleReviewAttestation(
  value: unknown,
): MathAuthoringOracleReviewAttestation {
  const item = object(value, "reviewAttestation", ["oracleConstraintSha256", "reviewedAt", "reviewer", "schemaVersion", "sourceFixture", "sourceFixtureId", "sourceSha256", "verdict"]);
  if (item.schemaVersion !== 2) throw new Error("reviewAttestation.schemaVersion: expected 2");
  return {
    oracleConstraintSha256: sha256Text(item.oracleConstraintSha256, "reviewAttestation.oracleConstraintSha256"),
    reviewedAt: calendarDate(item.reviewedAt, "reviewAttestation.reviewedAt"),
    reviewer: reviewIdentity(item.reviewer, "reviewAttestation.reviewer"),
    schemaVersion: 2,
    sourceFixture: text(item.sourceFixture, "reviewAttestation.sourceFixture"),
    sourceFixtureId: text(item.sourceFixtureId, "reviewAttestation.sourceFixtureId"),
    sourceSha256: sha256Text(item.sourceSha256, "reviewAttestation.sourceSha256"),
    verdict: choice(item.verdict, ["approved"], "reviewAttestation.verdict"),
  };
}

export function mathAuthoringOracleReviewAttestationDigest(
  attestation: MathAuthoringOracleReviewAttestation,
): string {
  return sha256(stableJson(attestation));
}

export function mathAuthoringOracleSourceDigest(
  source: MathAuthoringOracleSource,
): string {
  return sha256(stableJson(source));
}

function compilePreCapSemanticKey(
  constraint: MathAuthoringCaseConstraint,
  expected: NonNullable<MathAuthoringCaseConstraint["cap"]>["preCapRequiredHypotheses"][number],
  oracle: MathAuthoringOracle,
  anchors: Readonly<Record<string, ResolvedNamedAnchor>>,
): MathInterpretationPreCapSemanticKey {
  const hypothesis = requiredValue(
    constraint.advisory.requiredHypotheses.find((item) => item.id === expected.requiredHypothesisId),
    `${constraint.id}: pre-cap hypothesis ${expected.requiredHypothesisId} is not reviewed`,
  );
  if (hypothesis.supportAllowed.length !== 1) {
    throw new Error(`${constraint.id}: pre-cap hypothesis ${expected.requiredHypothesisId} requires one exact support tier`);
  }
  const formula = requiredValue(anchors[hypothesis.selector.formulaAnchor], `${constraint.id}: pre-cap formula anchor missing`);
  const evidence = hypothesis.evidence.map((evidenceId) => {
    const reviewed = requiredValue(oracle.evidence[evidenceId], `${constraint.id}: pre-cap evidence ${evidenceId} missing`);
    if (reviewed.provenance === undefined || reviewed.role === undefined) {
      throw new Error(`${constraint.id}: pre-cap evidence ${evidenceId} requires exact provenance and role`);
    }
    return {
      provenance: reviewed.provenance,
      role: reviewed.role,
      sourceAnchors: reviewed.anchors.map((anchorId) => {
        const anchor = requiredValue(anchors[anchorId], `${constraint.id}: pre-cap evidence anchor ${anchorId} missing`);
        return {
          documentVersion: anchor.documentVersion,
          generation: reviewed.generation,
          lifecycle: reviewed.lifecycle,
          location: anchor.location,
        };
      }),
    };
  });
  return {
    bindings: hypothesis.bindings.map(({ parameter, symbol }) => ({ parameter, symbol })),
    conditions: hypothesis.conditions.map(({ conditionId, status }) => ({ conditionId, status })),
    evidence,
    formulaSource: {
      documentVersion: formula.documentVersion,
      generation: expected.formulaGeneration,
      lifecycle: expected.formulaLifecycle,
      location: formula.location,
    },
    kind: hypothesis.selector.kind,
    label: hypothesis.selector.label,
    relationId: hypothesis.selector.relationId ?? null,
    support: hypothesis.supportAllowed[0]!,
  };
}

function sourceRelativeCapProjection(
  constraint: MathAuthoringCaseConstraint,
  oracle: MathAuthoringOracle,
  anchors: Readonly<Record<string, ResolvedNamedAnchor>>,
): unknown {
  const cap = requiredValue(constraint.cap, `${constraint.id}: cap projection missing`);
  return cap.preCapRequiredHypotheses.map((expected) => {
    const hypothesis = requiredValue(constraint.advisory.requiredHypotheses.find((item) => item.id === expected.requiredHypothesisId), `${constraint.id}: cap hypothesis missing`);
    return {
      bindings: hypothesis.bindings,
      conditions: hypothesis.conditions,
      evidence: hypothesis.evidence.map((id) => {
        const evidence = requiredValue(oracle.evidence[id], `${constraint.id}: cap evidence missing`);
        return {
          anchors: evidence.anchors.map((anchorId) => anchors[anchorId]?.logicalId),
          generation: evidence.generation,
          lifecycle: evidence.lifecycle,
          provenance: evidence.provenance,
          role: evidence.role,
        };
      }).sort(stableCompare),
      formulaAnchor: anchors[hypothesis.selector.formulaAnchor]?.logicalId,
      formulaGeneration: expected.formulaGeneration,
      formulaLifecycle: expected.formulaLifecycle,
      requiredHypothesisId: expected.requiredHypothesisId,
      kind: hypothesis.selector.kind,
      label: hypothesis.selector.label,
      relationId: hypothesis.selector.relationId ?? null,
      support: hypothesis.supportAllowed,
    };
  }).sort(stableCompare);
}

function sourceRelativeRequiredHypothesesProjection(
  constraint: MathAuthoringCaseConstraint,
  oracle: MathAuthoringOracle,
  anchors: Readonly<Record<string, ResolvedNamedAnchor>>,
): unknown {
  return constraint.advisory.requiredHypotheses.map((hypothesis) => ({
    bindings: [...hypothesis.bindings].sort(stableCompare),
    conditions: [...hypothesis.conditions].sort(stableCompare),
    dependentFacets: [...hypothesis.dependentFacets].sort(),
    evidence: hypothesis.evidence.map((id) => {
      const evidence = requiredValue(oracle.evidence[id], `${constraint.id}: paired hypothesis evidence missing`);
      return {
        anchors: evidence.anchors.map((anchorId) =>
          requiredValue(anchors[anchorId], `${constraint.id}: paired hypothesis evidence anchor missing`).logicalId
        ).sort(),
        generation: evidence.generation,
        kind: evidence.kind,
        lifecycle: evidence.lifecycle,
        provenance: evidence.provenance ?? null,
        role: evidence.role ?? null,
        ruleId: evidence.ruleId ?? null,
        strength: evidence.strength,
      };
    }).sort(stableCompare),
    id: hypothesis.id,
    releaseRequired: hypothesis.releaseRequired,
    selector: {
      formulaAnchor: requiredValue(
        anchors[hypothesis.selector.formulaAnchor],
        `${constraint.id}: paired hypothesis formula anchor missing`,
      ).logicalId,
      kind: hypothesis.selector.kind,
      label: hypothesis.selector.label,
      relationId: hypothesis.selector.relationId ?? null,
    },
    supportAllowed: [...hypothesis.supportAllowed].sort(),
  })).sort(stableCompare);
}

function sourceRelativePairSafetyProjection(
  constraint: MathAuthoringCaseConstraint,
  oracle: MathAuthoringOracle,
  anchors: Readonly<Record<string, ResolvedNamedAnchor>>,
): unknown {
  const anchor = (anchorId: string, label: string) =>
    requiredValue(anchors[anchorId], `${constraint.id}: paired ${label} anchor missing`).logicalId;
  const selector = (item: HypothesisSelector) => ({
    ...item,
    formulaAnchor: anchor(item.formulaAnchor, "selector formula"),
  });
  const evidence = (id: string) => {
    const reviewed = requiredValue(
      oracle.evidence[id],
      `${constraint.id}: paired safety evidence ${id} missing`,
    );
    return {
      anchors: reviewed.anchors.map((anchorId) =>
        requiredValue(
          anchors[anchorId],
          `${constraint.id}: paired safety evidence anchor missing`,
        ).logicalId
      ).sort(),
      generation: reviewed.generation,
      kind: reviewed.kind,
      lifecycle: reviewed.lifecycle,
      provenance: reviewed.provenance ?? null,
      role: reviewed.role ?? null,
      ruleId: reviewed.ruleId ?? null,
      strength: reviewed.strength,
    };
  };
  return {
    allowedExtras: {
      ...constraint.advisory.allowedExtras,
      anchorAllowlist: constraint.advisory.allowedExtras.anchorAllowlist.map((anchorId) =>
        anchor(anchorId, "allowed-extra")
      ).sort(),
    },
    disposition: constraint.safety.disposition,
    forbiddenAuthority: constraint.safety.forbiddenAuthority.map(selector).sort(stableCompare),
    formulaAnchor: anchor(constraint.safety.formulaAnchor, "safety formula"),
    interpretationsTruncated: constraint.safety.interpretationsTruncated,
    lifecycle: { ...constraint.safety.lifecycle, documentVersion: undefined },
    limits: constraint.safety.limits.map((limit) => ({
      evidence: limit.evidence.map(evidence).sort(stableCompare),
      kind: limit.kind,
    })).sort(stableCompare),
    noUnexpectedAuthority: constraint.safety.noUnexpectedAuthority,
    noUnexpectedContradictions: constraint.safety.noUnexpectedContradictions,
    requiredAuthority: [...constraint.safety.requiredAuthority].sort(),
    requiredContradictions: [...constraint.safety.requiredContradictions].sort(),
    transition: constraint.transition ? {
      after: {
        ...constraint.transition.after,
        forbiddenAnchors: constraint.transition.after.forbiddenAnchors.map((id) =>
          anchor(id, "transition forbidden")
        ).sort(),
        forbiddenAuthority: constraint.transition.after.forbiddenAuthority.map(selector).sort(stableCompare),
        formulaAnchor: anchor(constraint.transition.after.formulaAnchor, "transition after formula"),
        lifecycle: { ...constraint.transition.after.lifecycle, documentVersion: undefined },
        requiredMissingDiscriminators: [...constraint.transition.after.requiredMissingDiscriminators].sort(stableCompare),
      },
      before: {
        ...constraint.transition.before,
        formulaAnchor: anchor(constraint.transition.before.formulaAnchor, "transition before formula"),
        lifecycle: { ...constraint.transition.before.lifecycle, documentVersion: undefined },
        requiredAnchors: constraint.transition.before.requiredAnchors.map((id) =>
          anchor(id, "transition required")
        ).sort(),
        requiredAuthority: [...constraint.transition.before.requiredAuthority].sort(),
      },
      removed: constraint.transition.removed ? {
        ...constraint.transition.removed,
        selectionAnchor: anchor(constraint.transition.removed.selectionAnchor, "transition removed selection"),
      } : undefined,
    } : undefined,
    truncated: constraint.safety.truncated,
  };
}

export function mathAuthoringExpectedObservationPlan(
  compiled: CompiledMathAuthoringOracle,
): readonly MathAuthoringExpectedObservation[] {
  const planned = new Map<string, MathAuthoringExpectedObservation>();
  const add = (
    constraint: MathAuthoringCaseConstraint,
    snapshotId: string,
    mode: "clean" | "incremental",
    selectionAnchorId: string,
    context: "absent" | "present",
  ): void => {
    const key = `${constraint.id}:${snapshotId}:${mode}`;
    const selection = requiredValue(compiled.anchors[selectionAnchorId], `${key}: compiled observation selection missing`);
    const entry: MathAuthoringExpectedObservation = {
      caseId: constraint.id,
      context,
      mode,
      selection,
      selectionAnchorId,
      snapshotId,
      sourceCaseId: constraint.sourceCaseId,
    };
    const prior = planned.get(key);
    if (prior && stableJson(prior) !== stableJson(entry)) throw new Error(`${key}: conflicting compiled observation plan`);
    if (!prior) planned.set(key, entry);
  };
  for (const constraint of compiled.oracle.cases) {
    const sourceCase = requiredValue(compiled.source.cases.find((item) => item.id === constraint.sourceCaseId), `${constraint.id}: compiled source case missing`);
    const primary = requiredValue(sourceCase.selections.find((selection) => selection.id === constraint.selectionId), `${constraint.id}: compiled selection missing`);
    for (const mode of ["clean", "incremental"] as const) add(constraint, primary.snapshotId, mode, `${sourceCase.id}:${primary.anchor}`, "present");
    if (constraint.transition) for (const mode of ["clean", "incremental"] as const) {
      const beforeSelection = requiredValue(
        sourceCase.selections.find((selection) => selection.snapshotId === constraint.transition!.before.snapshotId),
        `${constraint.id}: transition before selection missing`,
      );
      const afterSelection = requiredValue(
        sourceCase.selections.find((selection) => selection.snapshotId === constraint.transition!.after.snapshotId),
        `${constraint.id}: transition after selection missing`,
      );
      add(constraint, constraint.transition.before.snapshotId, mode, `${sourceCase.id}:${beforeSelection.anchor}`, "present");
      add(constraint, constraint.transition.after.snapshotId, mode, `${sourceCase.id}:${afterSelection.anchor}`, "present");
      if (constraint.transition.removed) add(constraint, constraint.transition.removed.snapshotId, mode, constraint.transition.removed.selectionAnchor, "absent");
    }
  }
  return [...planned.values()];
}

export function evaluateMathAuthoringOracle(
  compiled: CompiledMathAuthoringOracle,
  observations: readonly MathAuthoringOracleObservation[],
): MathAuthoringOracleReport {
  const safetyFailures: string[] = [];
  const advisoryFindings: string[] = [];
  const transitionFailures: string[] = [];
  const suppressed = new Set<MathAuthoringOracleFacet>();
  const expectedPlan = mathAuthoringExpectedObservationPlan(compiled);
  const expectedObservations = new Map(expectedPlan.map((item) => [`${item.caseId}:${item.snapshotId}:${item.mode}`, item]));
  const byKey = new Map<string, MathAuthoringOracleObservation>();
  for (const observation of observations) {
    const key = `${observation.caseId}:${observation.snapshotId}:${observation.mode}`;
    const expected = expectedObservations.get(key);
    if (expected === undefined) {
      safetyFailures.push(`observations: unexpected ${key}`);
      continue;
    }
    if (byKey.has(key)) {
      safetyFailures.push(`observations: duplicate ${key}`);
      continue;
    }
    if (!sameResolvedAnchor(observation.selection.location, observation.selection.documentVersion, expected.selection)) {
      safetyFailures.push(`${key}: selection receipt mismatch`);
      continue;
    }
    if (observation.authoringContext) {
      try {
        parseObservedMathAuthoringContext(observation.authoringContext, `${key}.authoringContext`);
      } catch (error) {
        safetyFailures.push(`${key}: malformed authoring context: ${error instanceof Error ? error.message : String(error)}`);
        continue;
      }
    }
    byKey.set(key, observation);
  }
  for (const key of expectedObservations.keys()) if (!byKey.has(key)) safetyFailures.push(`observations: missing ${key}`);
  const constraintById = new Map(compiled.oracle.cases.map((constraint) => [constraint.id, constraint]));
  for (const expected of expectedPlan) {
    if (expected.context !== "present") continue;
    const context = byKey.get(`${expected.caseId}:${expected.snapshotId}:${expected.mode}`)?.authoringContext;
    if (!context) continue;
    validateCandidateCapEnvelope(
      context,
      requiredValue(constraintById.get(expected.caseId), `${expected.caseId}: compiled constraint missing`).cap !== undefined,
      `${expected.caseId}:${expected.snapshotId}:${expected.mode}`,
      safetyFailures,
    );
  }
  for (const expected of expectedPlan) {
    if (expected.context !== "present" || expected.mode !== "clean") continue;
    const clean = byKey.get(`${expected.caseId}:${expected.snapshotId}:clean`)?.authoringContext;
    const incremental = byKey.get(`${expected.caseId}:${expected.snapshotId}:incremental`)?.authoringContext;
    if (!clean || !incremental || compareMathAuthoringContext(projectMathAuthoringContext(clean), incremental).length) {
      transitionFailures.push(`${expected.caseId}: ${expected.snapshotId} clean/incremental mismatch`);
    }
  }
  for (const constraint of compiled.oracle.cases) {
    const sourceCase = requiredValue(
      compiled.source.cases.find((item) => item.id === constraint.sourceCaseId),
      `${constraint.id}: compiled source case missing`,
    );
    const snapshotId = requiredValue(
      sourceCase.selections.find(
        (selection) => selection.id === constraint.selectionId,
      ),
      `${constraint.id}: compiled selection missing`,
    ).snapshotId;
    const observation = byKey.get(`${constraint.id}:${snapshotId}:clean`);
    if (!observation?.authoringContext) {
      safetyFailures.push(`${constraint.id}: missing current clean authoring context`);
      continue;
    }
    const context = observation.authoringContext;
    safetyFailures.push(...mathAuthoringContextSafetyFailures(context).map(formatSafety));
    evaluateSafety(constraint, context, compiled, safetyFailures);
    evaluateAdvisory(constraint, context, compiled, safetyFailures, advisoryFindings, suppressed);
    if (constraint.cap) evaluateCap(constraint, context, compiled, safetyFailures);
    if (constraint.transition) evaluateTransition(constraint, compiled, byKey, transitionFailures);
  }
  const pairFailures = evaluatePairs(compiled, byKey);
  const diagnostic = mathAuthoringDiagnosticArtifact(observations);
  return { advisoryFindings, diagnostic: { artifactId: diagnostic.artifactId, sha256: diagnostic.sha256 }, pairFailures, safetyFailures, suppressedFacets: [...suppressed].sort(), transitionFailures };
}

export function mathAuthoringDiagnosticArtifact(
  observations: readonly MathAuthoringOracleObservation[],
): { readonly artifactId: string; readonly content: string; readonly sha256: string } {
  const content = stableJson(observations.map((item) => ({
    caseId: item.caseId,
    mode: item.mode,
    selection: item.selection,
    ...(item.authoringContext ? { stable: projectMathAuthoringContext(item.authoringContext) } : {}),
    snapshotId: item.snapshotId,
  })).sort(stableCompare)) + "\n";
  const digest = sha256(content);
  return { artifactId: `sha256:${digest}`, content, sha256: digest };
}

export function mathAuthoringDiagnosticArtifactPath(
  artifact: { readonly artifactId: string; readonly sha256: string },
): string {
  if (artifact.artifactId !== `sha256:${artifact.sha256}` || !/^[0-9a-f]{64}$/u.test(artifact.sha256)) {
    throw new Error("diagnostic artifact: invalid content address");
  }
  return `.artifacts/math-authoring-oracle/${artifact.sha256}.json`;
}

export function isMathAuthoringRemovedContextSafelyAbsent(value: unknown): boolean {
  if (value === undefined) return true;
  try {
    const top = object(value, "removedContext", ["claimEvidence", "conditions", "disposition", "equationLinks", "lifecycle", "interpretations", "notationOccurrences", "requirements", "truncated"], ["approximation", "conventionalCandidates", "formula"]);
    const interpretations = object(top.interpretations, "removedContext.interpretations", ["analysisLimits", "exhaustiveness", "hypotheses", "missingDiscriminators", "truncated"], ["candidateCap"]);
    const context = parseObservedMathAuthoringContext(value, "removedContext");
    return context.formula === undefined &&
      context.approximation === undefined &&
      (context.conventionalCandidates?.length ?? 0) === 0 &&
      context.claimEvidence.length === 0 &&
      context.conditions.length === 0 &&
      context.equationLinks.length === 0 &&
      context.notationOccurrences.length === 0 &&
      context.requirements.length === 0 &&
      context.disposition === "unsupported" &&
      context.truncated === false &&
      context.lifecycle.capped === false &&
      context.lifecycle.engineLimited === false &&
      context.lifecycle.retracted === false &&
      interpretations.candidateCap === undefined &&
      context.interpretations.analysisLimits.length === 0 &&
      context.interpretations.hypotheses.length === 0 &&
      context.interpretations.missingDiscriminators.length === 0 &&
      context.interpretations.truncated === false;
  } catch {
    return false;
  }
}

function evaluateSafety(constraint: MathAuthoringCaseConstraint, context: MathAuthoringContext, compiled: CompiledMathAuthoringOracle, failures: string[]): void {
  const prefix = constraint.id;
  const formula = requiredValue(
    compiled.anchors[constraint.safety.formulaAnchor],
    `${prefix}: compiled formula anchor missing`,
  );
  if (!context.formula || !sameLocation(context.formula.location, formula.location) || context.formula.documentVersion !== formula.documentVersion) failures.push(`${prefix}: formula anchor mismatch`);
  if (stableJson(context.lifecycle) !== stableJson(constraint.safety.lifecycle)) failures.push(`${prefix}: lifecycle mismatch`);
  if (context.disposition !== constraint.safety.disposition) failures.push(`${prefix}: disposition ${context.disposition} != ${constraint.safety.disposition}`);
  if (context.truncated !== constraint.safety.truncated ||
    context.interpretations.truncated !== constraint.safety.interpretationsTruncated) {
    failures.push(`${prefix}: truncation mismatch`);
  }
  const actualLimits = context.interpretations.analysisLimits.map((limit) => ({ evidence: evidenceReferencesKey(limit.evidence, compiled), kind: limit.kind })).sort(stableCompare);
  const expectedLimits = constraint.safety.limits.map((limit) => ({ evidence: limit.evidence.map((id) => evidenceConstraintProjection(compiled.oracle.evidence[id]!, compiled)), kind: limit.kind })).sort(stableCompare);
  if (stableJson(actualLimits) !== stableJson(expectedLimits)) failures.push(`${prefix}: exact analysis limits mismatch`);
  const matchedAuthority = new Set<MathInterpretationHypothesisInfo>();
  for (const id of constraint.safety.requiredAuthority) {
    const expected = requiredValue(
      constraint.advisory.requiredHypotheses.find((item) => item.id === id),
      `${prefix}: compiled required authority ${id} missing`,
    );
    const matches = context.interpretations.hypotheses.filter((item) =>
      isMathematicalAuthority(item.kind, item.support) && requiredHypothesisMatches(item, expected, compiled)
    );
    if (matches.length !== 1) {
      failures.push(`${prefix}: required authority ${id} expected one exact authority, found ${matches.length}`);
    } else {
      matchedAuthority.add(matches[0]!);
    }
  }
  for (const selector of constraint.safety.forbiddenAuthority) if (context.interpretations.hypotheses.some((item) => hypothesisMatches(item, selector, compiled) && isMathematicalAuthority(item.kind, item.support))) failures.push(`${prefix}: forbidden authority ${selectorKey(selector)}`);
  if (constraint.safety.noUnexpectedAuthority) {
    for (const hypothesis of context.interpretations.hypotheses.filter((item) => isMathematicalAuthority(item.kind, item.support))) if (!matchedAuthority.has(hypothesis)) failures.push(`${prefix}: unexpected authority ${hypothesis.kind}/${hypothesis.label}/${hypothesis.support}`);
  }
  const matchedContradictions = new Set<MathInterpretationHypothesisInfo>();
  for (const id of constraint.safety.requiredContradictions) {
    const expected = requiredValue(constraint.advisory.requiredHypotheses.find((item) => item.id === id), `${prefix}: compiled contradiction ${id} missing`);
    const matches = context.interpretations.hypotheses.filter((item) => item.support === "contradicted" && requiredHypothesisMatches(item, expected, compiled));
    if (matches.length !== 1) failures.push(`${prefix}: required contradiction ${id} expected one exact contradiction, found ${matches.length}`);
    else matchedContradictions.add(matches[0]!);
  }
  if (constraint.safety.noUnexpectedContradictions) {
    for (const hypothesis of context.interpretations.hypotheses.filter((item) => item.support === "contradicted")) if (!matchedContradictions.has(hypothesis)) failures.push(`${prefix}: unexpected contradiction ${hypothesis.kind}/${hypothesis.label}`);
  }
  const claims = context.claimEvidence.map((item) => ({ anchor: logicalAnchorForLocation(item.claim, context.lifecycle.documentVersion, compiled), modality: item.modality, polarity: item.polarity, strengthCeiling: item.strengthCeiling })).sort(stableCompare);
  const expectedClaims = constraint.safety.claims.map((item) => ({ ...item, anchor: compiled.anchors[item.anchor]?.logicalId ?? item.anchor })).sort(stableCompare);
  if (stableJson(claims) !== stableJson(expectedClaims)) failures.push(`${prefix}: exact claim safety projection mismatch`);
  if (constraint.safety.approximation) {
    const actual = context.approximation;
    const anchor = requiredValue(
      compiled.anchors[constraint.safety.approximation.relationAnchor],
      `${prefix}: compiled approximation anchor missing`,
    );
    if (!actual || actual.exactness !== "approximate" || !sameRange(actual.relationRange, anchor.location.range)) failures.push(`${prefix}: approximation safety projection mismatch`);
  } else if (context.approximation) failures.push(`${prefix}: unexpected approximation authority`);
  const links = context.equationLinks.map((item) => ({
    kind: item.kind,
    sourceAnchor: logicalAnchorForLocation(item.source.location, item.source.documentVersion, compiled),
    targetAnchor: logicalAnchorForLocation(item.target.location, item.target.documentVersion, compiled),
  })).sort(stableCompare);
  const expectedLinks = constraint.safety.equationLinks.map((item) => ({
    kind: item.kind,
    sourceAnchor: compiled.anchors[item.sourceAnchor]?.logicalId,
    targetAnchor: compiled.anchors[item.targetAnchor]?.logicalId,
  })).sort(stableCompare);
  if (stableJson(links) !== stableJson(expectedLinks)) failures.push(`${prefix}: exact equation-link safety projection mismatch`);
  for (const generated of constraint.safety.generatedSubnodes) {
    const anchor = requiredValue(compiled.anchors[generated.anchor], `${prefix}: compiled generated anchor missing`);
    for (const evidenceId of generated.evidence) {
      const expected = requiredValue(compiled.oracle.evidence[evidenceId], `${prefix}: compiled generated evidence missing`);
      const found = context.interpretations.hypotheses.some((hypothesis) =>
        hypothesis.evidence.some((evidence) =>
          interpretationEvidenceMatches(evidence, expected, compiled) &&
          evidence.sourceAnchors.some((sourceAnchor) =>
            sourceAnchor.generation === "generated" &&
            sameResolvedAnchor(sourceAnchor.location, sourceAnchor.documentVersion, anchor)
          )
        )
      );
      if (!found) failures.push(`${prefix}: generated subnode ${generated.anchor} missing exact generated provenance ${evidenceId}`);
    }
  }
  const notation = context.notationOccurrences.map((item) => ({
    anchor: logicalAnchorForLocation(item.location, item.occurrenceId.documentVersion, compiled),
    sourceNotation: item.sourceNotation,
  })).sort(stableCompare);
  const expectedNotation = constraint.safety.notation.map((item) => ({
    anchor: compiled.anchors[item.anchor]?.logicalId,
    sourceNotation: item.sourceNotation,
  })).sort(stableCompare);
  if (stableJson(notation) !== stableJson(expectedNotation)) failures.push(`${prefix}: exact notation safety projection mismatch`);
}

function evaluateAdvisory(constraint: MathAuthoringCaseConstraint, context: MathAuthoringContext, compiled: CompiledMathAuthoringOracle, safety: string[], advisory: string[], suppressed: Set<MathAuthoringOracleFacet>): void {
  const matched = new Set<MathInterpretationHypothesisInfo>();
  const byId = new Map<string, MathInterpretationHypothesisInfo>();
  for (const required of constraint.advisory.requiredHypotheses) {
    const hypothesis = context.interpretations.hypotheses.find((item) => hypothesisMatches(item, required.selector, compiled));
    if (!hypothesis) {
      const message = `${constraint.id}: missing hypothesis ${required.id}`;
      if (required.releaseRequired) safety.push(message); else advisory.push(message);
      for (const facet of required.dependentFacets) suppressed.add(facet);
      continue;
    }
    matched.add(hypothesis); byId.set(required.id, hypothesis);
    if (!required.supportAllowed.includes(hypothesis.support)) safety.push(`${constraint.id}: ${required.id} support ${hypothesis.support} outside reviewed bounds`);
    if (!requiredHypothesisMatches(hypothesis, required, compiled)) safety.push(`${constraint.id}: ${required.id} does not exactly match reviewed support/evidence/bindings/conditions`);
    if (!requiredBindingConstraintsMatch(hypothesis.bindings, required.bindings)) safety.push(`${constraint.id}: ${required.id} bindings mismatch`);
    if (!requiredConditionConstraintsMatch(hypothesis.conditions, required.conditions)) safety.push(`${constraint.id}: ${required.id} conditions mismatch`);
  }
  for (const order of constraint.advisory.relativeOrder) {
    const before = byId.get(order.before); const after = byId.get(order.after);
    if (before && after && before.rank >= after.rank) safety.push(`${constraint.id}: required relative order ${order.before} before ${order.after}`);
  }
  const extras = context.interpretations.hypotheses.filter((item) => !matched.has(item));
  const allowed = constraint.advisory.allowedExtras;
  if (extras.length > allowed.maxCount) safety.push(`${constraint.id}: advisory extras ${extras.length} exceed ${allowed.maxCount}`);
  for (const extra of extras) {
    if (!allowed.kinds.includes(extra.kind) ||
      !allowed.supportAllowed.includes(extra.support) ||
      extra.evidence.length === 0 ||
      extra.evidence.some((item) =>
        !allowed.provenances.includes(item.provenance) ||
        item.sourceAnchors.length === 0 ||
        item.sourceAnchors.some((anchor) =>
          !allowed.anchorAllowlist.some((id) =>
            sameResolvedAnchor(
              anchor.location,
              anchor.documentVersion,
              compiled.anchors[id],
            )
          )
        )
      )) {
      safety.push(`${constraint.id}: unreviewed advisory extra ${extra.kind}/${extra.label}`);
    }
  }
  for (const requirement of constraint.advisory.requiredRequirements) if (!context.requirements.some((item) => requirementMatches(item, requirement))) advisory.push(`${constraint.id}: missing requirement ${requirementKey(requirement)}`);
  for (const requirement of constraint.advisory.requiredMissingDiscriminators) if (!context.interpretations.missingDiscriminators.some((item) => requirementMatches(item, requirement))) advisory.push(`${constraint.id}: missing discriminator ${requirementKey(requirement)}`);
}

function evaluateCap(
  constraint: MathAuthoringCaseConstraint,
  context: MathAuthoringContext,
  compiled: CompiledMathAuthoringOracle,
  failures: string[],
): void {
  const cap = requiredValue(constraint.cap, `${constraint.id}: compiled cap constraint missing`);
  const expected = requiredValue(compiled.capExpectations[constraint.id], `${constraint.id}: compiled cap expectation missing`);
  const metadata = context.interpretations.candidateCap;
  if (!metadata) {
    failures.push(`${constraint.id}: missing protocol-owned candidateCap metadata`);
    return;
  }
  if (metadata.candidateCountBeforeCap !== expected.candidateCountBeforeCap ||
    metadata.preCapSemanticKeyDigest !== expected.preCapSemanticKeyDigest) {
    failures.push(`${constraint.id}: candidateCap count/digest mismatch`);
  }
  if (context.interpretations.hypotheses.length !== cap.exposedExact ||
    !context.interpretations.truncated || !context.truncated || !context.lifecycle.capped) {
    failures.push(`${constraint.id}: cap+1 envelope mismatch`);
  }
  const correct = requiredValue(
    constraint.advisory.requiredHypotheses.find((item) => item.id === cap.correctHypothesisId),
    `${constraint.id}: compiled correct cap hypothesis missing`,
  );
  if (!context.interpretations.hypotheses[0] || !requiredHypothesisMatches(context.interpretations.hypotheses[0], correct, compiled)) {
    failures.push(`${constraint.id}: correct interpretation did not survive first`);
  }
  const actualKeys = context.interpretations.hypotheses.map((hypothesis) => preCapSemanticKeyFromHypothesis(hypothesis, context.lifecycle));
  const actualCanonical = canonicalKeyStrings(actualKeys);
  const expectedCanonical = new Set(canonicalKeyStrings(expected.semanticKeys));
  if (actualCanonical.length !== context.interpretations.hypotheses.length) failures.push(`${constraint.id}: post-cap hypotheses contain duplicate semantic keys`);
  for (const key of actualCanonical) if (!expectedCanonical.has(key)) failures.push(`${constraint.id}: exposed hypothesis absent from reviewed pre-cap identities`);
  if (![...expectedCanonical].some((key) => !actualCanonical.includes(key))) failures.push(`${constraint.id}: no reviewed alternative was actually truncated`);
  const kinds = context.interpretations.analysisLimits.map((item) => item.kind);
  for (const kind of cap.requiredLimitKinds) if (!kinds.includes(kind)) failures.push(`${constraint.id}: missing cap limit ${kind}`);
}

function validateCandidateCapEnvelope(
  context: MathAuthoringContext,
  capExpected: boolean,
  prefix: string,
  failures: string[],
): void {
  const metadata = context.interpretations.candidateCap;
  const capLimits = context.interpretations.analysisLimits.filter((limit) => limit.kind === "candidate-set-capped");
  const completeEnvelope = capLimits.length === 1 &&
    context.truncated && context.interpretations.truncated && context.lifecycle.capped;
  if ((metadata !== undefined) !== (capLimits.length > 0) || capLimits.length > 1) {
    failures.push(`${prefix}: candidateCap must exist iff exactly one candidate-set-capped limit is present`);
  }
  if ((metadata !== undefined) !== capExpected || (capLimits.length === 1) !== capExpected || completeEnvelope !== capExpected) {
    failures.push(`${prefix}: candidateCap envelope does not match the reviewed case contract`);
  }
  if (metadata && metadata.candidateCountBeforeCap > 0xffff_ffff) {
    failures.push(`${prefix}: candidateCountBeforeCap exceeds the u32 range`);
  }
}

function preCapSemanticKeyFromHypothesis(
  hypothesis: MathInterpretationHypothesisInfo,
  lifecycle: MathAuthoringContext["lifecycle"],
): MathInterpretationPreCapSemanticKey {
  const formula = requiredValue(hypothesis.formula, `hypothesis ${hypothesis.label}: cap semantic key requires formula source`);
  return {
    bindings: hypothesis.bindings.map(({ parameter, symbol }) => ({ parameter, symbol })),
    conditions: hypothesis.conditions.map(({ conditionId, status }) => ({ conditionId, status })),
    evidence: hypothesis.evidence.map(({ provenance, role, sourceAnchors }) => ({
      provenance,
      role,
      sourceAnchors: sourceAnchors.map(({ documentVersion, generation, lifecycle, location }) => ({ documentVersion, generation, lifecycle, location })),
    })),
    formulaSource: {
      documentVersion: formula.documentVersion,
      generation: lifecycle.generation,
      lifecycle: lifecycle.retracted ? "retracted" : "current",
      location: formula.location,
    },
    kind: hypothesis.kind,
    label: hypothesis.label,
    relationId: hypothesis.relation?.relationId ?? null,
    support: hypothesis.support,
  };
}

function canonicalKeyStrings(values: readonly MathInterpretationPreCapSemanticKey[]): readonly string[] {
  const parsed = JSON.parse(canonicalMathInterpretationPreCapPayload(values)) as unknown;
  if (!Array.isArray(parsed) || parsed.some((value) => typeof value !== "string")) throw new Error("protocol pre-cap canonical payload must contain strings");
  return parsed as readonly string[];
}

function evaluateTransition(constraint: MathAuthoringCaseConstraint, compiled: CompiledMathAuthoringOracle, observations: ReadonlyMap<string, MathAuthoringOracleObservation>, failures: string[]): void {
  const transition = requiredValue(
    constraint.transition,
    `${constraint.id}: compiled transition missing`,
  );
  const before = observations.get(`${constraint.id}:${transition.before.snapshotId}:clean`)?.authoringContext;
  const after = observations.get(`${constraint.id}:${transition.after.snapshotId}:clean`)?.authoringContext;
  if (!before || !after) { failures.push(`${constraint.id}: missing before/after transition context`); return; }
  for (const [phase, context, expected] of [["before", before, transition.before], ["after", after, transition.after]] as const) {
    const formula = requiredValue(compiled.anchors[expected.formulaAnchor], `${constraint.id}: compiled ${phase} formula anchor missing`);
    if (!context.formula || !sameResolvedAnchor(context.formula.location, context.formula.documentVersion, formula)) failures.push(`${constraint.id}: ${phase} formula anchor mismatch`);
    if (stableJson(context.lifecycle) !== stableJson(expected.lifecycle)) failures.push(`${constraint.id}: ${phase} lifecycle mismatch`);
    if (context.disposition !== expected.disposition) failures.push(`${constraint.id}: ${phase} disposition mismatch`);
  }
  for (const id of transition.before.requiredAuthority) {
    const expected = requiredValue(constraint.advisory.requiredHypotheses.find((item) => item.id === id), `${constraint.id}: compiled before authority missing`);
    if (!before.interpretations.hypotheses.some((item) => isMathematicalAuthority(item.kind, item.support) && requiredHypothesisMatches(item, expected, compiled))) failures.push(`${constraint.id}: before missing exact reviewed authority ${id}`);
  }
  for (const id of transition.before.requiredAnchors) if (!contextHasAnchor(before, requiredValue(compiled.anchors[id], `${constraint.id}: compiled transition anchor missing`))) failures.push(`${constraint.id}: before missing anchor ${id}`);
  for (const selector of transition.after.forbiddenAuthority) if (after.interpretations.hypotheses.some((item) => hypothesisMatches(item, selector, compiled) && isMathematicalAuthority(item.kind, item.support))) failures.push(`${constraint.id}: after retained removed authority ${selectorKey(selector)}`);
  if (constraint.safety.noUnexpectedAuthority) {
    for (const hypothesis of after.interpretations.hypotheses.filter((item) =>
      isMathematicalAuthority(item.kind, item.support)
    )) failures.push(`${constraint.id}: after retained unexpected authority ${hypothesis.kind}/${hypothesis.label}/${hypothesis.support}`);
  }
  for (const id of transition.after.forbiddenAnchors) if (contextHasAnchor(after, requiredValue(compiled.anchors[id], `${constraint.id}: compiled transition anchor missing`))) failures.push(`${constraint.id}: after retained removed anchor ${id}`);
  for (const requirement of transition.after.requiredMissingDiscriminators) if (!after.interpretations.missingDiscriminators.some((item) => requirementMatches(item, requirement))) failures.push(`${constraint.id}: after missing reviewed discriminator ${requirementKey(requirement)}`);
  if (transition.removed) {
    for (const mode of ["clean", "incremental"] as const) {
      const observation = observations.get(`${constraint.id}:${transition.removed.snapshotId}:${mode}`);
      if (!observation) failures.push(`${constraint.id}: removed snapshot missing ${mode} absence observation`);
      else if (!isMathAuthoringRemovedContextSafelyAbsent(observation.authoringContext)) failures.push(`${constraint.id}: removed formula still exposes stale ${mode} math-authoring state`);
    }
  }
}

function evaluatePairs(compiled: CompiledMathAuthoringOracle, observations: ReadonlyMap<string, MathAuthoringOracleObservation>): string[] {
  const failures: string[] = [];
  const cases = new Map(compiled.oracle.cases.map((item) => [item.id, item]));
  const sources = new Map(compiled.source.cases.map((item) => [item.id, item]));
  for (const pair of compiled.oracle.pairs) {
    const texCase = requiredValue(cases.get(pair.texCaseId), `${pair.id}: compiled TeX case missing`);
    const mdCase = requiredValue(cases.get(pair.markdownCaseId), `${pair.id}: compiled Markdown case missing`);
    const texSource = requiredValue(sources.get(texCase.sourceCaseId), `${pair.id}: compiled TeX source missing`);
    const mdSource = requiredValue(sources.get(mdCase.sourceCaseId), `${pair.id}: compiled Markdown source missing`);
    const texSnapshot = requiredValue(texSource.selections.find((selection) => selection.id === texCase.selectionId), `${pair.id}: compiled TeX selection missing`).snapshotId;
    const mdSnapshot = requiredValue(mdSource.selections.find((selection) => selection.id === mdCase.selectionId), `${pair.id}: compiled Markdown selection missing`).snapshotId;
    const presentSnapshots = new Set([
      texSnapshot,
      ...(texCase.transition ? [texCase.transition.before.snapshotId, texCase.transition.after.snapshotId] : []),
    ]);
    for (const snapshotId of presentSnapshots) {
      const pairedSnapshotId = snapshotId === texSnapshot ? mdSnapshot : snapshotId;
      const tex = observations.get(`${texCase.id}:${snapshotId}:clean`)?.authoringContext;
      const md = observations.get(`${mdCase.id}:${pairedSnapshotId}:clean`)?.authoringContext;
      if (!tex || !md || stableJson(pairProjection(tex, pair.compare.hypotheses, compiled, texCase)) !== stableJson(pairProjection(md, pair.compare.hypotheses, compiled, mdCase))) {
        failures.push(`${pair.id}: TeX/Markdown semantic parity mismatch`);
        break;
      }
    }
  }
  return failures;
}

function pairProjection(
  context: MathAuthoringContext,
  fields: readonly string[],
  compiled: CompiledMathAuthoringOracle,
  constraint: MathAuthoringCaseConstraint,
): unknown {
  const projectHypothesis = (item: MathInterpretationHypothesisInfo) => ({
    ...(fields.includes("kind") ? { kind: item.kind } : {}),
    ...(fields.includes("label") ? { label: item.label } : {}),
    ...(fields.includes("relationId") ? { relationId: item.relation?.relationId } : {}),
    ...(fields.includes("formulaAnchor") ? { formulaAnchor: item.formula ? logicalAnchorForLocation(item.formula.location, item.formula.documentVersion, compiled) : null } : {}),
    ...(fields.includes("support") ? { support: item.support } : {}),
    ...(fields.includes("bindings") ? { bindings: item.bindings.map(({ parameter, symbol }) => ({ parameter, symbol })).sort(stableCompare) } : {}),
    ...(fields.includes("conditions") ? { conditions: item.conditions.map(({ conditionId, label, status }) => ({ conditionId, label, status })).sort(stableCompare) } : {}),
    ...(fields.includes("evidence") ? { evidence: item.evidence.map((evidence) => ({ kind: evidence.evidence.kind, logicalAnchors: evidence.sourceAnchors.map((anchor) => ({ anchor: logicalAnchorForLocation(anchor.location, anchor.documentVersion, compiled), generation: anchor.generation, lifecycle: anchor.lifecycle })).sort(stableCompare), provenance: evidence.provenance, role: evidence.role, ruleId: evidence.evidence.ruleId, strength: evidence.evidence.strength })).sort(stableCompare) } : {}),
  });
  const requiredOrder = constraint.advisory.requiredHypotheses
    .flatMap((required) => {
      const hypothesis = context.interpretations.hypotheses.find((item) =>
        hypothesisMatches(item, required.selector, compiled)
      );
      return hypothesis ? [{ id: required.id, rank: hypothesis.rank }] : [];
    })
    .sort((left, right) => left.rank - right.rank)
    .map((item) => item.id);
  return {
    authoringTruncated: context.truncated,
    candidateCapCount: context.interpretations.candidateCap?.candidateCountBeforeCap,
    disposition: context.disposition,
    hypotheses: context.interpretations.hypotheses.map(projectHypothesis).sort(stableCompare),
    lifecycle: { ...context.lifecycle, documentVersion: undefined },
    limits: context.interpretations.analysisLimits.map((item) => ({ kind: item.kind, evidence: evidenceReferencesKey(item.evidence, compiled) })).sort(stableCompare),
    requiredOrder,
    truncated: context.interpretations.truncated,
  };
}

function parseEvidenceConstraint(value: unknown, path: string): EvidenceConstraint {
  const item = object(value, path, ["anchors", "generation", "kind", "lifecycle", "strength"], ["provenance", "role", "ruleId"]);
  return { anchors: strings(item.anchors, `${path}.anchors`), generation: choice(item.generation, ["authored", "generated"], `${path}.generation`), kind: text(item.kind, `${path}.kind`), lifecycle: choice(item.lifecycle, ["current", "retracted"], `${path}.lifecycle`), ...(item.provenance === undefined ? {} : { provenance: choice(item.provenance, provenances, `${path}.provenance`) }), ...(item.role === undefined ? {} : { role: choice(item.role, ["supporting", "contradicting"], `${path}.role`) }), ...(item.ruleId === undefined ? {} : { ruleId: text(item.ruleId, `${path}.ruleId`) }), strength: text(item.strength, `${path}.strength`) };
}

function parseCaseConstraint(value: unknown, index: number): MathAuthoringCaseConstraint {
  const path = `oracle.cases[${index}]`; const item = object(value, path, ["advisory", "facets", "id", "safety", "selectionId", "sourceCaseId"], ["cap", "transition"]);
  const safety = object(item.safety, `${path}.safety`, ["claims", "disposition", "equationLinks", "forbiddenAuthority", "formulaAnchor", "generatedSubnodes", "interpretationsTruncated", "lifecycle", "limits", "noUnexpectedAuthority", "noUnexpectedContradictions", "notation", "requiredAuthority", "requiredContradictions", "truncated"], ["approximation"]);
  const advisory = object(item.advisory, `${path}.advisory`, ["allowedExtras", "coverageGoals", "knownMisses", "relativeOrder", "requiredHypotheses", "requiredMissingDiscriminators", "requiredRequirements"]);
  const allowed = object(advisory.allowedExtras, `${path}.advisory.allowedExtras`, ["anchorAllowlist", "kinds", "maxCount", "provenances", "supportAllowed"]);
  const requiredHypotheses = array(advisory.requiredHypotheses, `${path}.advisory.requiredHypotheses`).map((value, hIndex) => parseRequiredHypothesis(value, `${path}.advisory.requiredHypotheses[${hIndex}]`));
  unique(requiredHypotheses.map((entry) => entry.id), `${path}.advisory.requiredHypotheses.id`);
  const parsedLifecycle = parseLifecycle(safety.lifecycle, `${path}.safety.lifecycle`);
  const cap = item.cap === undefined
    ? undefined
    : (() => {
        const entry = object(item.cap, `${path}.cap`, ["correctHypothesisId", "exposedExact", "preCapRequiredHypotheses", "requiredLimitKinds"]);
        const preCapRequiredHypotheses = array(entry.preCapRequiredHypotheses, `${path}.cap.preCapRequiredHypotheses`).map((value, capIndex) => {
          const capPath = `${path}.cap.preCapRequiredHypotheses[${capIndex}]`;
          const expected = object(value, capPath, ["formulaGeneration", "formulaLifecycle", "requiredHypothesisId"]);
          return {
            formulaGeneration: choice(expected.formulaGeneration, ["authored", "generated"], `${capPath}.formulaGeneration`),
            formulaLifecycle: choice(expected.formulaLifecycle, ["current", "retracted"], `${capPath}.formulaLifecycle`),
            requiredHypothesisId: text(expected.requiredHypothesisId, `${capPath}.requiredHypothesisId`),
          };
        });
        unique(preCapRequiredHypotheses.map((expected) => expected.requiredHypothesisId), `${path}.cap.preCapRequiredHypotheses.requiredHypothesisId`);
        return { correctHypothesisId: text(entry.correctHypothesisId, `${path}.cap.correctHypothesisId`), exposedExact: positive(entry.exposedExact, `${path}.cap.exposedExact`), preCapRequiredHypotheses, requiredLimitKinds: choices(entry.requiredLimitKinds, ["candidate-set-capped", "evidence-truncated", "discriminator-set-capped", "engine-limit", "generated-source", "retracted-source"], `${path}.cap.requiredLimitKinds`) };
      })();
  const transition = item.transition === undefined
    ? undefined
    : parseTransition(item.transition, `${path}.transition`);
  const result: MathAuthoringCaseConstraint = {
    advisory: {
      allowedExtras: { anchorAllowlist: strings(allowed.anchorAllowlist, `${path}.advisory.allowedExtras.anchorAllowlist`), kinds: choices(allowed.kinds, hypothesisKinds, `${path}.advisory.allowedExtras.kinds`), maxCount: nonnegative(allowed.maxCount, `${path}.advisory.allowedExtras.maxCount`), provenances: choices(allowed.provenances, provenances, `${path}.advisory.allowedExtras.provenances`), supportAllowed: choices(allowed.supportAllowed, supportTiers, `${path}.advisory.allowedExtras.supportAllowed`) },
      coverageGoals: array(advisory.coverageGoals, `${path}.advisory.coverageGoals`).map((value, goalIndex) => { const entryPath = `${path}.advisory.coverageGoals[${goalIndex}]`; const entry = object(value, entryPath, ["facet", "rationale"]); return { facet: choice(entry.facet, facets, `${entryPath}.facet`), rationale: text(entry.rationale, `${entryPath}.rationale`) }; }),
      knownMisses: array(advisory.knownMisses, `${path}.advisory.knownMisses`).map((value, missIndex) => { const entryPath = `${path}.advisory.knownMisses[${missIndex}]`; const entry = object(value, entryPath, ["facet", "rationale"]); return { facet: choice(entry.facet, facets, `${entryPath}.facet`), rationale: text(entry.rationale, `${entryPath}.rationale`) }; }),
      relativeOrder: array(advisory.relativeOrder, `${path}.advisory.relativeOrder`).map((value, orderIndex) => { const entryPath = `${path}.advisory.relativeOrder[${orderIndex}]`; const entry = object(value, entryPath, ["after", "before"]); return { after: text(entry.after, `${entryPath}.after`), before: text(entry.before, `${entryPath}.before`) }; }),
      requiredHypotheses,
      requiredMissingDiscriminators: array(advisory.requiredMissingDiscriminators, `${path}.advisory.requiredMissingDiscriminators`).map((value, rIndex) => parseRequirementConstraint(value, `${path}.advisory.requiredMissingDiscriminators[${rIndex}]`)),
      requiredRequirements: array(advisory.requiredRequirements, `${path}.advisory.requiredRequirements`).map((value, rIndex) => parseRequirementConstraint(value, `${path}.advisory.requiredRequirements[${rIndex}]`)),
    },
    facets: choices(item.facets, facets, `${path}.facets`), id: text(item.id, `${path}.id`),
    safety: {
      ...(safety.approximation === undefined ? {} : { approximation: (() => { const entry = object(safety.approximation, `${path}.safety.approximation`, ["exactness", "relationAnchor"]); return { exactness: choice(entry.exactness, ["approximate"], `${path}.safety.approximation.exactness`), relationAnchor: text(entry.relationAnchor, `${path}.safety.approximation.relationAnchor`) }; })() }),
      claims: array(safety.claims, `${path}.safety.claims`).map((value, claimIndex) => { const entryPath = `${path}.safety.claims[${claimIndex}]`; const entry = object(value, entryPath, ["anchor", "modality", "polarity", "strengthCeiling"]); return { anchor: text(entry.anchor, `${entryPath}.anchor`), modality: text(entry.modality, `${entryPath}.modality`), polarity: text(entry.polarity, `${entryPath}.polarity`), strengthCeiling: text(entry.strengthCeiling, `${entryPath}.strengthCeiling`) }; }),
      disposition: choice(safety.disposition, ["established", "partial", "conventional", "ambiguous", "conflicting", "unsupported", "engine-limited"], `${path}.safety.disposition`),
      equationLinks: array(safety.equationLinks, `${path}.safety.equationLinks`).map((value, linkIndex) => { const entryPath = `${path}.safety.equationLinks[${linkIndex}]`; const entry = object(value, entryPath, ["kind", "sourceAnchor", "targetAnchor"]); return { kind: text(entry.kind, `${entryPath}.kind`), sourceAnchor: text(entry.sourceAnchor, `${entryPath}.sourceAnchor`), targetAnchor: text(entry.targetAnchor, `${entryPath}.targetAnchor`) }; }),
      forbiddenAuthority: array(safety.forbiddenAuthority, `${path}.safety.forbiddenAuthority`).map((value, selectorIndex) => parseSelector(value, `${path}.safety.forbiddenAuthority[${selectorIndex}]`)), formulaAnchor: text(safety.formulaAnchor, `${path}.safety.formulaAnchor`),
      generatedSubnodes: array(safety.generatedSubnodes, `${path}.safety.generatedSubnodes`).map((value, generatedIndex) => { const entryPath = `${path}.safety.generatedSubnodes[${generatedIndex}]`; const entry = object(value, entryPath, ["anchor", "evidence"]); return { anchor: text(entry.anchor, `${entryPath}.anchor`), evidence: strings(entry.evidence, `${entryPath}.evidence`) }; }),
      interpretationsTruncated: bool(safety.interpretationsTruncated, `${path}.safety.interpretationsTruncated`), lifecycle: parsedLifecycle,
      limits: array(safety.limits, `${path}.safety.limits`).map((value, limitIndex) => { const entryPath = `${path}.safety.limits[${limitIndex}]`; const entry = object(value, entryPath, ["evidence", "kind"]); return { evidence: strings(entry.evidence, `${entryPath}.evidence`), kind: text(entry.kind, `${entryPath}.kind`) }; }), noUnexpectedAuthority: bool(safety.noUnexpectedAuthority, `${path}.safety.noUnexpectedAuthority`), noUnexpectedContradictions: bool(safety.noUnexpectedContradictions, `${path}.safety.noUnexpectedContradictions`),
      notation: array(safety.notation, `${path}.safety.notation`).map((value, nIndex) => { const entryPath = `${path}.safety.notation[${nIndex}]`; const entry = object(value, entryPath, ["anchor", "sourceNotation"]); return { anchor: text(entry.anchor, `${entryPath}.anchor`), sourceNotation: text(entry.sourceNotation, `${entryPath}.sourceNotation`) }; }),
      requiredAuthority: strings(safety.requiredAuthority, `${path}.safety.requiredAuthority`), requiredContradictions: strings(safety.requiredContradictions, `${path}.safety.requiredContradictions`), truncated: bool(safety.truncated, `${path}.safety.truncated`),
    }, selectionId: text(item.selectionId, `${path}.selectionId`), sourceCaseId: text(item.sourceCaseId, `${path}.sourceCaseId`),
    ...(cap ? { cap } : {}),
    ...(transition ? { transition } : {}),
  };
  return result;
}

function parseRequiredHypothesis(value: unknown, path: string): RequiredHypothesisConstraint {
  const item = object(value, path, ["bindings", "conditions", "dependentFacets", "evidence", "id", "releaseRequired", "selector", "supportAllowed"]);
  const bindings = array(item.bindings, `${path}.bindings`).map((bindingValue, index) => parseRequiredBinding(bindingValue, `${path}.bindings[${index}]`));
  const conditions = array(item.conditions, `${path}.conditions`).map((conditionValue, index) => parseRequiredCondition(conditionValue, `${path}.conditions[${index}]`));
  const selector = parseSelector(item.selector, `${path}.selector`);
  if (selector.kind === "source-meaning" && (bindings.length !== 0 || conditions.length !== 0)) {
    throw new Error(`${path}: source-meaning hypotheses must have empty bindings and conditions`);
  }
  unique(bindings.map((binding) => stableJson(binding)), `${path}.bindings`);
  unique(conditions.map((condition) => condition.conditionId), `${path}.conditions.conditionId`);
  return {
    bindings,
    conditions,
    dependentFacets: choices(item.dependentFacets, facets, `${path}.dependentFacets`),
    evidence: strings(item.evidence, `${path}.evidence`),
    id: text(item.id, `${path}.id`),
    releaseRequired: bool(item.releaseRequired, `${path}.releaseRequired`),
    selector,
    supportAllowed: choices(item.supportAllowed, supportTiers, `${path}.supportAllowed`),
  };
}
function parseRequiredBinding(value: unknown, path: string): RequiredHypothesisConstraint["bindings"][number] {
  const item = object(value, path, ["parameter", "symbol"]);
  return {
    parameter: text(item.parameter, `${path}.parameter`),
    symbol: text(item.symbol, `${path}.symbol`),
  };
}
function parseRequiredCondition(value: unknown, path: string): RequiredHypothesisConstraint["conditions"][number] {
  const item = object(value, path, ["conditionId", "label", "status"]);
  return {
    conditionId: text(item.conditionId, `${path}.conditionId`),
    label: text(item.label, `${path}.label`),
    status: choice(item.status, ["conflicting", "required", "unsupported", "verified"], `${path}.status`),
  };
}
function parseSelector(value: unknown, path: string): HypothesisSelector {
  const item = object(value, path, ["formulaAnchor", "kind", "label"], ["relationId"]);
  const kind = choice(item.kind, hypothesisKinds, `${path}.kind`);
  const label = text(item.label, `${path}.label`);
  const relationId = item.relationId === undefined ? undefined : text(item.relationId, `${path}.relationId`);
  if ((kind === "typed-law" || kind === "reviewed-convention") && relationId === undefined) {
    throw new Error(`${path}.relationId: ${kind} requires stable relation identity`);
  }
  return {
    formulaAnchor: text(item.formulaAnchor, `${path}.formulaAnchor`),
    kind,
    label,
    ...(relationId === undefined ? {} : { relationId }),
  };
}
function parseRequirementConstraint(value: unknown, path: string): RequirementConstraint { const item = object(value, path, ["kind"], ["conditionLabel", "parameter", "symbol"]); return { ...(item.conditionLabel === undefined ? {} : { conditionLabel: text(item.conditionLabel, `${path}.conditionLabel`) }), kind: choice(item.kind, ["declaration", "role-declaration", "condition", "disambiguation"], `${path}.kind`), ...(item.parameter === undefined ? {} : { parameter: text(item.parameter, `${path}.parameter`) }), ...(item.symbol === undefined ? {} : { symbol: text(item.symbol, `${path}.symbol`) }) }; }
function parseLifecycle(value: unknown, path: string): MathAuthoringContext["lifecycle"] {
  const lifecycle = object(value, path, ["capped", "documentVersion", "editable", "engineLimited", "freshness", "generation", "retracted"]);
  return {
    capped: bool(lifecycle.capped, `${path}.capped`),
    documentVersion: positive(lifecycle.documentVersion, `${path}.documentVersion`),
    editable: bool(lifecycle.editable, `${path}.editable`),
    engineLimited: bool(lifecycle.engineLimited, `${path}.engineLimited`),
    freshness: choice(lifecycle.freshness, ["current"], `${path}.freshness`),
    generation: choice(lifecycle.generation, ["authored", "generated"], `${path}.generation`),
    retracted: bool(lifecycle.retracted, `${path}.retracted`),
  };
}

function parseTransition(value: unknown, path: string): NonNullable<MathAuthoringCaseConstraint["transition"]> {
  const item = object(value, path, ["after", "before", "cleanIncremental"], ["removed"]);
  if (item.cleanIncremental !== true) throw new Error(`${path}.cleanIncremental: expected true`);
  const before = object(item.before, `${path}.before`, ["disposition", "formulaAnchor", "lifecycle", "requiredAnchors", "requiredAuthority", "snapshotId"]);
  const after = object(item.after, `${path}.after`, ["disposition", "forbiddenAnchors", "forbiddenAuthority", "formulaAnchor", "lifecycle", "requiredMissingDiscriminators", "snapshotId"]);
  const removed = item.removed === undefined ? undefined : object(item.removed, `${path}.removed`, ["context", "selectionAnchor", "snapshotId"]);
  return {
    after: {
      disposition: choice(after.disposition, ["established", "partial", "conventional", "ambiguous", "conflicting", "unsupported", "engine-limited"], `${path}.after.disposition`),
      forbiddenAnchors: strings(after.forbiddenAnchors, `${path}.after.forbiddenAnchors`),
      forbiddenAuthority: array(after.forbiddenAuthority, `${path}.after.forbiddenAuthority`).map((entry, index) => parseSelector(entry, `${path}.after.forbiddenAuthority[${index}]`)),
      formulaAnchor: text(after.formulaAnchor, `${path}.after.formulaAnchor`),
      lifecycle: parseLifecycle(after.lifecycle, `${path}.after.lifecycle`),
      requiredMissingDiscriminators: array(after.requiredMissingDiscriminators, `${path}.after.requiredMissingDiscriminators`).map((entry, index) => parseRequirementConstraint(entry, `${path}.after.requiredMissingDiscriminators[${index}]`)),
      snapshotId: text(after.snapshotId, `${path}.after.snapshotId`),
    },
    before: {
      disposition: choice(before.disposition, ["established", "partial", "conventional", "ambiguous", "conflicting", "unsupported", "engine-limited"], `${path}.before.disposition`),
      formulaAnchor: text(before.formulaAnchor, `${path}.before.formulaAnchor`),
      lifecycle: parseLifecycle(before.lifecycle, `${path}.before.lifecycle`),
      requiredAnchors: strings(before.requiredAnchors, `${path}.before.requiredAnchors`),
      requiredAuthority: strings(before.requiredAuthority, `${path}.before.requiredAuthority`),
      snapshotId: text(before.snapshotId, `${path}.before.snapshotId`),
    },
    cleanIncremental: true,
    ...(removed ? {
      removed: {
        context: choice(removed.context, ["absent"], `${path}.removed.context`),
        selectionAnchor: text(removed.selectionAnchor, `${path}.removed.selectionAnchor`),
        snapshotId: text(removed.snapshotId, `${path}.removed.snapshotId`),
      },
    } : {}),
  };
}

function validateConstraintReferences(
  item: MathAuthoringCaseConstraint,
  oracle: MathAuthoringOracle,
  anchors: Readonly<Record<string, ResolvedNamedAnchor>>,
): void {
  const selectors = [
    ...item.safety.forbiddenAuthority,
    ...item.advisory.requiredHypotheses.map((entry) => entry.selector),
    ...(item.transition?.after.forbiddenAuthority ?? []),
  ];
  const anchorIds = [
    item.safety.formulaAnchor,
    ...selectors.flatMap((selector) => selector.formulaAnchor ? [selector.formulaAnchor] : []),
    ...item.safety.claims.map((entry) => entry.anchor),
    ...item.safety.equationLinks.flatMap((entry) => [entry.sourceAnchor, entry.targetAnchor]),
    ...item.safety.generatedSubnodes.map((entry) => entry.anchor),
    ...item.safety.notation.map((entry) => entry.anchor),
    ...(item.safety.approximation ? [item.safety.approximation.relationAnchor] : []),
    ...item.advisory.allowedExtras.anchorAllowlist,
    ...(item.transition ? [item.transition.before.formulaAnchor, item.transition.after.formulaAnchor] : []),
    ...(item.transition?.before.requiredAnchors ?? []),
    ...(item.transition?.after.forbiddenAnchors ?? []),
    ...(item.transition?.removed ? [item.transition.removed.selectionAnchor] : []),
  ];
  for (const id of anchorIds) {
    const anchor = anchors[id];
    if (!anchor) throw new Error(`${item.id}: unknown anchor ${id}`);
    if (anchor.caseId !== item.sourceCaseId) {
      throw new Error(`${item.id}: anchor ${id} belongs to another source case`);
    }
  }
  const evidenceIds = [
    ...item.safety.limits.flatMap((entry) => entry.evidence),
    ...item.safety.generatedSubnodes.flatMap((entry) => entry.evidence),
    ...item.advisory.requiredHypotheses.flatMap((entry) => entry.evidence),
  ];
  for (const id of evidenceIds) {
    const evidence = oracle.evidence[id];
    if (!evidence) throw new Error(`${item.id}: unknown evidence ${id}`);
    for (const anchorId of evidence.anchors) {
      const anchor = anchors[anchorId];
      if (!anchor || anchor.caseId !== item.sourceCaseId) {
        throw new Error(`${item.id}: evidence ${id} has an unknown or cross-case anchor ${anchorId}`);
      }
    }
  }
  for (const generated of item.safety.generatedSubnodes) {
    const formula = requiredValue(anchors[item.safety.formulaAnchor], `${item.id}: safety formula anchor missing`);
    const subnode = requiredValue(anchors[generated.anchor], `${item.id}: generated subnode anchor missing`);
    if (generated.anchor === item.safety.formulaAnchor ||
      formula.snapshotId !== subnode.snapshotId ||
      formula.fileId !== subnode.fileId ||
      formula.documentVersion !== subnode.documentVersion ||
      formula.location.path !== subnode.location.path ||
      subnode.location.range.startOffset < formula.location.range.startOffset ||
      subnode.location.range.endOffset > formula.location.range.endOffset ||
      sameRange(subnode.location.range, formula.location.range)) {
      throw new Error(`${item.id}: generated subnode ${generated.anchor} must be a strict nested descendant of the safety formula`);
    }
    if (!generated.evidence.length) throw new Error(`${item.id}: generated subnode requires reviewed evidence`);
    for (const id of generated.evidence) {
      const evidence = requiredValue(oracle.evidence[id], `${item.id}: generated subnode evidence missing`);
      if (evidence.generation !== "generated" || !evidence.anchors.includes(generated.anchor)) {
        throw new Error(`${item.id}: generated subnode evidence ${id} must identify its generated anchor`);
      }
    }
  }
  const hypothesisIds = new Set(
    item.advisory.requiredHypotheses.map((entry) => entry.id),
  );
  const authorityIds = [
    ...item.safety.requiredAuthority,
    ...(item.transition?.before.requiredAuthority ?? []),
  ];
  for (const id of authorityIds) {
    const expected = item.advisory.requiredHypotheses.find((entry) => entry.id === id);
    if (!expected) throw new Error(`${item.id}: required authority references unknown hypothesis ${id}`);
    if (!expected.releaseRequired || expected.supportAllowed.some((support) =>
      !isMathematicalAuthority(expected.selector.kind, support)
    )) {
      throw new Error(`${item.id}: required authority ${id} must be release-required and authority-bearing`);
    }
  }
  for (const id of item.safety.requiredContradictions) {
    const expected = item.advisory.requiredHypotheses.find((entry) => entry.id === id);
    if (!expected || !expected.releaseRequired || stableJson(expected.supportAllowed) !== stableJson(["contradicted"])) {
      throw new Error(`${item.id}: required contradiction ${id} must be release-required and contradicted-only`);
    }
  }
  if (item.safety.requiredAuthority.some((id) => item.safety.requiredContradictions.includes(id))) {
    throw new Error(`${item.id}: authority and contradiction identities must be disjoint`);
  }
  for (const order of item.advisory.relativeOrder) {
    if (!hypothesisIds.has(order.before) || !hypothesisIds.has(order.after)) {
      throw new Error(`${item.id}: relative order references unknown hypothesis`);
    }
  }
  if (item.cap && !hypothesisIds.has(item.cap.correctHypothesisId)) {
    throw new Error(`${item.id}: cap correctHypothesisId is unknown`);
  }
  const candidateCapLimits = item.safety.limits.filter((limit) => limit.kind === "candidate-set-capped");
  const completeCapEnvelope = candidateCapLimits.length === 1 &&
    item.safety.truncated && item.safety.interpretationsTruncated && item.safety.lifecycle.capped;
  if ((item.cap === undefined && candidateCapLimits.length !== 0) ||
    (item.cap !== undefined && !completeCapEnvelope)) {
    throw new Error(`${item.id}: cap must exist iff one candidate-set-capped limit and both truncation flags plus lifecycle.capped are present`);
  }
  if (item.cap && !item.cap.requiredLimitKinds.includes("candidate-set-capped")) {
    throw new Error(`${item.id}: cap requiredLimitKinds must include candidate-set-capped`);
  }
  if (item.cap) {
    const preCapIds = item.cap.preCapRequiredHypotheses.map((expected) => expected.requiredHypothesisId);
    for (const id of preCapIds) if (!hypothesisIds.has(id)) throw new Error(`${item.id}: pre-cap identity references unknown hypothesis ${id}`);
    if (!preCapIds.includes(item.cap.correctHypothesisId)) throw new Error(`${item.id}: correct hypothesis must be one of the reviewed pre-cap identities`);
  }
  if (item.transition) {
    if (!item.transition.after.forbiddenAnchors.includes(item.transition.before.formulaAnchor)) {
      throw new Error(`${item.id}: transition after must forbid the before formula anchor`);
    }
    for (const [phase, value] of [["before", item.transition.before], ["after", item.transition.after]] as const) {
      const anchor = requiredValue(anchors[value.formulaAnchor], `${item.id}: transition ${phase} formula anchor missing`);
      if (anchor.snapshotId !== value.snapshotId || anchor.documentVersion !== value.lifecycle.documentVersion) {
        throw new Error(`${item.id}: transition ${phase} formula/lifecycle snapshot mismatch`);
      }
    }
    if (item.transition.removed) {
      const cursor = requiredValue(anchors[item.transition.removed.selectionAnchor], `${item.id}: removed selection anchor missing`);
      if (cursor.snapshotId !== item.transition.removed.snapshotId) {
        throw new Error(`${item.id}: removed selection anchor snapshot mismatch`);
      }
    }
  }
}

function hypothesisMatches(item: MathInterpretationHypothesisInfo, selector: HypothesisSelector, compiled: CompiledMathAuthoringOracle): boolean { return item.kind === selector.kind && (selector.label === undefined || item.label === selector.label) && (selector.relationId === undefined || item.relation?.relationId === selector.relationId) && (selector.formulaAnchor === undefined || (!!item.formula && sameResolvedAnchor(item.formula.location, item.formula.documentVersion, compiled.anchors[selector.formulaAnchor]))); }
function requiredHypothesisMatches(item: MathInterpretationHypothesisInfo, expected: RequiredHypothesisConstraint, compiled: CompiledMathAuthoringOracle): boolean {
  if (!hypothesisMatches(item, expected.selector, compiled) || !expected.supportAllowed.includes(item.support)) return false;
  if (!requiredBindingConstraintsMatch(item.bindings, expected.bindings)) return false;
  if (!requiredConditionConstraintsMatch(item.conditions, expected.conditions)) return false;
  if (item.evidence.length !== expected.evidence.length) return false;
  const remaining = [...item.evidence];
  for (const evidenceId of expected.evidence) {
    const evidence = requiredValue(compiled.oracle.evidence[evidenceId], `compiled evidence ${evidenceId} missing`);
    const index = remaining.findIndex((actual) => interpretationEvidenceMatches(actual, evidence, compiled));
    if (index < 0) return false;
    remaining.splice(index, 1);
  }
  return remaining.length === 0;
}
function requiredBindingConstraintsMatch(actual: MathInterpretationHypothesisInfo["bindings"], expected: RequiredHypothesisConstraint["bindings"]): boolean {
  return stableJson(actual.map(({ parameter, symbol }) => ({ parameter, symbol })).sort(stableCompare)) === stableJson([...expected].sort(stableCompare));
}
function requiredConditionConstraintsMatch(actual: MathInterpretationHypothesisInfo["conditions"], expected: RequiredHypothesisConstraint["conditions"]): boolean {
  return stableJson(actual.map(({ conditionId, label, status }) => ({ conditionId, label, status })).sort(stableCompare)) === stableJson([...expected].sort(stableCompare));
}
function interpretationEvidenceMatches(item: MathInterpretationEvidenceInfo, expected: EvidenceConstraint, compiled: CompiledMathAuthoringOracle): boolean { return item.evidence.kind === expected.kind && (expected.ruleId === undefined || item.evidence.ruleId === expected.ruleId) && item.evidence.strength === expected.strength && (expected.provenance === undefined || item.provenance === expected.provenance) && (expected.role === undefined || item.role === expected.role) && sameAnchorSet(item.sourceAnchors, expected, compiled); }
function sameAnchorSet(actual: readonly { documentVersion: number; generation: "authored" | "generated"; lifecycle: "current" | "retracted"; location: { fileId: string; path: string; range: SourceRange } }[], expected: EvidenceConstraint, compiled: CompiledMathAuthoringOracle): boolean { return stableJson(actual.map((item) => ({ anchor: logicalAnchorForLocation(item.location, item.documentVersion, compiled), generation: item.generation, lifecycle: item.lifecycle })).sort(stableCompare)) === stableJson(expected.anchors.map((id) => ({ anchor: compiled.anchors[id]?.logicalId, generation: expected.generation, lifecycle: expected.lifecycle })).sort(stableCompare)); }
function evidenceReferencesKey(items: readonly MathInterpretationEvidenceReferenceInfo[], compiled: CompiledMathAuthoringOracle): unknown { return items.map((item) => ({ kind: item.evidence.kind, ruleId: item.evidence.ruleId, strength: item.evidence.strength, anchors: item.sourceAnchors.map((anchor) => ({ anchor: logicalAnchorForLocation(anchor.location, anchor.documentVersion, compiled), generation: anchor.generation, lifecycle: anchor.lifecycle })).sort(stableCompare) })).sort(stableCompare); }
function evidenceConstraintProjection(item: EvidenceConstraint, compiled: CompiledMathAuthoringOracle): unknown { return { anchors: item.anchors.map((id) => ({ anchor: compiled.anchors[id]?.logicalId, generation: item.generation, lifecycle: item.lifecycle })).sort(stableCompare), kind: item.kind, ruleId: item.ruleId, strength: item.strength }; }
function requirementMatches(item: MathInterpretationRequirementInfo, expected: RequirementConstraint): boolean { if (item.kind !== expected.kind) return false; if (expected.symbol !== undefined && (!("symbol" in item) || item.symbol !== expected.symbol)) return false; if (expected.parameter !== undefined && (item.kind !== "role-declaration" || item.parameter !== expected.parameter)) return false; return expected.conditionLabel === undefined || (item.kind === "condition" && item.condition.label === expected.conditionLabel); }
function contextHasAnchor(context: MathAuthoringContext, anchor: ResolvedNamedAnchor): boolean { let found = false; const visit = (value: unknown): void => { if (found || !value) return; if (Array.isArray(value)) { value.forEach(visit); return; } if (typeof value !== "object") return; const item = value as Record<string, unknown>; if (isLocation(item.location) && typeof item.documentVersion === "number" && sameResolvedAnchor(item.location, item.documentVersion, anchor)) found = true; Object.values(item).forEach(visit); }; visit(context); return found; }
function logicalAnchorForLocation(location: { fileId: string; path: string; range: SourceRange }, version: number, compiled: CompiledMathAuthoringOracle): string | null { return Object.values(compiled.anchors).find((anchor) => sameResolvedAnchor(location, version, anchor))?.logicalId ?? null; }
function sameResolvedAnchor(location: { fileId: string; path: string; range: SourceRange }, version: number, anchor: ResolvedNamedAnchor | undefined): boolean { return !!anchor && version === anchor.documentVersion && sameLocation(location, anchor.location); }
function sameLocation(left: { fileId: string; path: string; range: SourceRange }, right: { fileId: string; path: string; range: SourceRange }): boolean { return left.fileId === right.fileId && left.path === right.path && sameRange(left.range, right.range); }
function sameRange(left: SourceRange, right: SourceRange): boolean { return left.startOffset === right.startOffset && left.endOffset === right.endOffset; }
function assertFormulaContainsSelection(
  caseId: string,
  sourceCase: MathAuthoringOracleSourceCase,
  formula: ResolvedNamedAnchor | undefined,
  selection: ResolvedNamedAnchor | undefined,
  phase: string,
): void {
  if (!formula || !selection ||
    formula.snapshotId !== selection.snapshotId ||
    formula.fileId !== selection.fileId ||
    formula.documentVersion !== selection.documentVersion ||
    formula.location.path !== selection.location.path ||
    formula.location.range.startOffset >= selection.location.range.startOffset ||
    formula.location.range.endOffset <= selection.location.range.endOffset) {
    throw new Error(`${caseId}: ${phase} formula must strictly contain its source selection in the same document snapshot`);
  }
  const snapshot = sourceCase.snapshots.find((item) => item.id === formula.snapshotId);
  const document = snapshot?.documents.find((item) => item.fileId === formula.fileId);
  if (!document) throw new Error(`${caseId}: ${phase} formula document is missing`);
  const leading = document.content.slice(
    formula.location.range.startOffset,
    selection.location.range.startOffset,
  );
  const trailing = document.content.slice(
    selection.location.range.endOffset,
    formula.location.range.endOffset,
  );
  if (!/^\s+$/u.test(leading) || !/^\s+$/u.test(trailing)) {
    throw new Error(`${caseId}: ${phase} formula margins outside the source selection must be whitespace only`);
  }
}
function isMathematicalAuthority(
  kind: MathInterpretationHypothesisInfo["kind"],
  support: MathInterpretationHypothesisInfo["support"],
): boolean {
  if (support === "explicit" || support === "derived") return true;
  return support === "supported" &&
    (kind === "typed-law" || kind === "source-meaning" || kind === "reviewed-convention");
}
function selectorKey(value: HypothesisSelector): string { return stableJson(value); }
function requirementKey(value: RequirementConstraint): string { return stableJson(value); }
function formatSafety(value: MathAuthoringContextFailure): string { return `${value.path}: ${value.kind}`; }
function isLocation(value: unknown): value is { fileId: string; path: string; range: SourceRange } { return !!value && typeof value === "object" && typeof (value as Record<string, unknown>).fileId === "string" && typeof (value as Record<string, unknown>).path === "string" && !!(value as Record<string, unknown>).range; }

function occurrenceRange(content: string, needle: string, occurrence: number | undefined, path: string, within: SourceRange): SourceRange { const segment = content.slice(within.startOffset, within.endOffset); const positions: number[] = []; let from = 0; while (true) { const index = segment.indexOf(needle, from); if (index < 0) break; positions.push(within.startOffset + index); from = index + Math.max(1, needle.length); } if (!positions.length) throw new Error(`${path}: needle not found`); if (occurrence === undefined && positions.length !== 1) throw new Error(`${path}: needle must be unique or declare occurrence`); const selected = positions[(occurrence ?? 1) - 1]; if (selected === undefined) throw new Error(`${path}: occurrence exceeds matches`); return { startOffset: selected, endOffset: selected + needle.length }; }
function uniqueNeedle(content: string, needle: string, occurrence: number | undefined, path: string): void { occurrenceRange(content, needle, occurrence, path, { startOffset: 0, endOffset: content.length }); }
function normalizedSourcePath(value: string, pathLabel: string): string {
  const slashPath = value.replaceAll("\\", "/");
  const normalized = path.normalize(slashPath);
  if (value !== slashPath || normalized !== slashPath || path.isAbsolute(normalized) || normalized === ".." || normalized.startsWith("../") || normalized === ".") {
    throw new Error(`${pathLabel}: expected a normalized repository-relative document path`);
  }
  return normalized;
}
function logicalFormatFileId(fileId: string): string { return fileId.replace(/\.(?:tex|md)$/u, ".<format>"); }
function sourceDependencyProjection(sourceCase: MathAuthoringOracleSourceCase): unknown {
  return sourceCase.snapshots.map((snapshot) => ({
    dependencies: snapshot.dependencies.map((dependency) => ({
      fromFileId: logicalFormatFileId(dependency.fromFileId),
      sourceAnchor: dependency.sourceAnchor,
      toFileId: logicalFormatFileId(dependency.toFileId),
    })).sort(stableCompare),
    documents: snapshot.documents.map((document) => ({
      fileId: logicalFormatFileId(document.fileId),
      path: logicalFormatFileId(normalizedSourcePath(document.path, `${sourceCase.id}.${snapshot.id}.path`)),
    })).sort(stableCompare),
    id: snapshot.id,
    mainFileId: logicalFormatFileId(snapshot.mainFileId),
  }));
}
function assertAcyclicDependencies(
  fileIds: ReadonlySet<string>,
  dependencies: readonly { readonly fromFileId: string; readonly toFileId: string }[],
  pathLabel: string,
): void {
  const outgoing = new Map([...fileIds].map((fileId) => [fileId, dependencies.filter((dependency) => dependency.fromFileId === fileId).map((dependency) => dependency.toFileId)]));
  const active = new Set<string>();
  const visited = new Set<string>();
  const visit = (fileId: string): void => {
    if (active.has(fileId)) throw new Error(`${pathLabel}: dependency cycle is forbidden`);
    if (visited.has(fileId)) return;
    active.add(fileId);
    for (const next of outgoing.get(fileId) ?? []) visit(next);
    active.delete(fileId);
    visited.add(fileId);
  };
  for (const fileId of fileIds) visit(fileId);
}
function assertReachableDependencies(
  mainFileId: string,
  fileIds: ReadonlySet<string>,
  dependencies: readonly { readonly fromFileId: string; readonly toFileId: string }[],
  pathLabel: string,
): void {
  const reachable = new Set<string>();
  const visit = (fileId: string): void => {
    if (reachable.has(fileId)) return;
    reachable.add(fileId);
    for (const dependency of dependencies) if (dependency.fromFileId === fileId) visit(dependency.toFileId);
  };
  visit(mainFileId);
  const disconnected = [...fileIds].filter((fileId) => !reachable.has(fileId)).sort();
  if (disconnected.length) throw new Error(`${pathLabel}: every document must be reachable from mainFileId; disconnected ${disconnected.join(", ")}`);
}
function rejectCanonicalWireObjects(value: unknown, path: string): void { if (Array.isArray(value)) return value.forEach((item, index) => rejectCanonicalWireObjects(item, `${path}[${index}]`)); if (!value || typeof value !== "object") return; for (const [key, child] of Object.entries(value)) { if (["hypothesisId", "requirementId", "candidateId", "alternativeId", "linkId", "claimId", "occurrenceGroup", "entityGroup", "hypothesisGroup", "StableMathAuthoringContext"].includes(key)) throw new Error(`${path}.${key}: raw wire identity/full stable context forbidden`); if ((key === "startOffset" || key === "endOffset") && path.startsWith("oracle")) throw new Error(`${path}.${key}: raw offsets forbidden in canonical oracle`); rejectCanonicalWireObjects(child, `${path}.${key}`); } }
function validateReviewAttestation(oracle: MathAuthoringOracle, value: unknown): void {
  if (oracle.review.reviewFixture !== canonicalReviewFixture) {
    throw new Error("oracle.review.reviewFixture: expected canonical review fixture path");
  }
  const attestation = parseMathAuthoringOracleReviewAttestation(value);
  const expectedConstraintDigest = mathAuthoringOracleConstraintDigest(oracle);
  if (attestation.oracleConstraintSha256 !== expectedConstraintDigest ||
    attestation.reviewer !== oracle.review.reviewer ||
    attestation.reviewedAt !== oracle.review.reviewedAt ||
    attestation.sourceFixture !== oracle.sourceFixture ||
    attestation.sourceFixtureId !== oracle.sourceFixtureId ||
    attestation.sourceSha256 !== oracle.sourceSha256) {
    throw new Error("reviewAttestation: source, reviewer, date, or oracle constraint binding mismatch");
  }
  if (mathAuthoringOracleReviewAttestationDigest(attestation) !== oracle.review.attestationDigest) {
    throw new Error("reviewAttestation: canonical attestation digest mismatch");
  }
}
function reviewIdentity(value: unknown, path: string): string {
  const identity = text(value, path);
  if (/pending|placeholder|self|unknown|tbd/iu.test(identity) ||
    !/^(?:agent:\/root\/[a-z0-9_/-]+|github:[a-z0-9_.-]+\/[a-z0-9_.-]+)$/iu.test(identity)) {
    throw new Error(`${path}: expected externally identifiable non-placeholder reviewer/author`);
  }
  return identity;
}
function canonicalReviewIdentity(value: string): string { return value.normalize("NFKC").toLocaleLowerCase("en-US"); }
function sha256Text(value: unknown, path: string): string {
  const digest = text(value, path);
  if (!/^[0-9a-f]{64}$/u.test(digest)) throw new Error(`${path}: expected sha256`);
  return digest;
}
function calendarDate(value: unknown, path: string): string {
  const date = text(value, path);
  const match = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(date);
  if (!match) throw new Error(`${path}: expected valid YYYY-MM-DD calendar date`);
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const parsed = new Date(Date.UTC(year, month - 1, day));
  if (parsed.getUTCFullYear() !== year || parsed.getUTCMonth() !== month - 1 || parsed.getUTCDate() !== day) {
    throw new Error(`${path}: expected valid YYYY-MM-DD calendar date`);
  }
  return date;
}
function parseMap<T>(value: unknown, path: string, parse: (value: unknown, path: string) => T): Readonly<Record<string, T>> { const input = record(value, path); return Object.fromEntries(Object.entries(input).map(([key, child]) => [text(key, `${path}.key`), parse(child, `${path}.${key}`)])); }
function object(value: unknown, path: string, required: readonly string[], optional: readonly string[] = []): Readonly<Record<string, unknown>> { const item = record(value, path); const allowed = new Set([...required, ...optional]); const extra = Object.keys(item).filter((key) => !allowed.has(key)); const missing = required.filter((key) => !(key in item)); if (extra.length) throw new Error(`${path}: unexpected keys ${extra.sort().join(", ")}`); if (missing.length) throw new Error(`${path}: missing keys ${missing.join(", ")}`); return item; }
function record(value: unknown, path: string): Readonly<Record<string, unknown>> { if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${path}: expected object`); return value as Readonly<Record<string, unknown>>; }
function array(value: unknown, path: string): readonly unknown[] { if (!Array.isArray(value)) throw new Error(`${path}: expected array`); return value; }
function text(value: unknown, path: string): string { if (typeof value !== "string" || !value.length) throw new Error(`${path}: expected non-empty string`); return value; }
function bool(value: unknown, path: string): boolean { if (typeof value !== "boolean") throw new Error(`${path}: expected boolean`); return value; }
function positive(value: unknown, path: string): number { if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) throw new Error(`${path}: expected positive integer`); return value; }
function nonnegative(value: unknown, path: string): number { if (typeof value !== "number" || !Number.isInteger(value) || value < 0) throw new Error(`${path}: expected nonnegative integer`); return value; }
function strings(value: unknown, path: string): readonly string[] { return array(value, path).map((item, index) => text(item, `${path}[${index}]`)); }
function choice<const T extends string>(value: unknown, choices: readonly T[], path: string): T { if (typeof value !== "string" || !choices.includes(value as T)) throw new Error(`${path}: expected ${choices.join(" or ")}`); return value as T; }
function choices<const T extends string>(value: unknown, allowed: readonly T[], path: string): readonly T[] { const result = array(value, path).map((item, index) => choice(item, allowed, `${path}[${index}]`)); unique(result, path); return result; }
function unique(values: readonly string[], path: string): void { const seen = new Set<string>(); for (const value of values) { if (seen.has(value)) throw new Error(`${path}: duplicate ${value}`); seen.add(value); } }
function stableCompare(left: unknown, right: unknown): number { return stableJson(left).localeCompare(stableJson(right)); }
function stableJson(value: unknown): string { if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`; if (value && typeof value === "object") return `{${Object.entries(value as Record<string, unknown>).filter(([, child]) => child !== undefined).sort(([left], [right]) => left.localeCompare(right)).map(([key, child]) => `${JSON.stringify(key)}:${stableJson(child)}`).join(",")}}`; return JSON.stringify(value); }
function sha256(value: string): string { return createHash("sha256").update(value).digest("hex"); }
function requiredValue<T>(value: T | undefined, message: string): T { if (value === undefined) throw new Error(message); return value; }
