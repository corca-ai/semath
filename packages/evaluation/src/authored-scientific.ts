import type {
  EntityId,
  EntitySurfaceAuthorization,
  MathAuthoringContext,
  MathAuthoringDisposition,
  MathInterpretationHypothesisInfo,
  MathInterpretationSetInfo,
  QueryResult,
  RelationInfo,
  SemanticViewInfo,
  SourceOccurrenceId,
  SourceRange,
} from "../../protocol/src/index";
import {
  parseMathAuthoringContextExpectation,
  type StableMathAuthoringContext,
} from "./math-authoring-development";
import { hypothesisIsMathematicalAuthority } from "./challenge-observation";
import type { CorpusDocument } from "./model";
import { roleInstancesMatch, type ObservedRole } from "./observation";

export const DOCUMENT_REASONING_FAMILIES = [
  "scope-comparison",
  "derivation-chain",
  "guarded-condition",
  "discourse-reference",
  "collision-unsupported",
  "edit-lifecycle",
] as const;

export const FIRST_LOSS_STAGES = [
  "neutral-syntax",
  "attachment",
  "identity",
  "canonical-ir",
  "typed-fact",
  "propagation",
  "pack-unification",
  "decision",
  "host-projection",
] as const;

export const AUTHORED_AREA_ALLOCATION = {
  "calculus-analysis": { development: 10, holdout: 6 },
  circuits: { development: 3, holdout: 1 },
  "classical-mechanics": { development: 3, holdout: 1 },
  "control-systems": { development: 3, holdout: 1 },
  "cross-field": { development: 18, holdout: 6 },
  "discrete-math": { development: 10, holdout: 6 },
  electromagnetism: { development: 10, holdout: 6 },
  "fluid-mechanics": { development: 10, holdout: 6 },
  "linear-algebra": { development: 3, holdout: 1 },
  "optimization-ml": { development: 10, holdout: 6 },
  probability: { development: 3, holdout: 1 },
  "signals-systems": { development: 3, holdout: 1 },
  "thermodynamics-heat-transfer": { development: 10, holdout: 6 },
} as const;

/** A commissioning target, not a quota that may override semantic review. */
export const AUTHORED_DECISION_TARGET = {
  ambiguous: 24,
  conflicting: 16,
  established: 56,
  partial: 36,
  unsupported: 12,
} as const satisfies Readonly<Record<ScientificDecision, number>>;

export type AuthoredArea = keyof typeof AUTHORED_AREA_ALLOCATION;
export type AuthoredSplit = "development" | "holdout";
export type DocumentReasoningFamily =
  (typeof DOCUMENT_REASONING_FAMILIES)[number];
export type FirstLossStage = (typeof FIRST_LOSS_STAGES)[number];
export type ScientificDecision = SemanticViewInfo["decision"]["status"];

export type AuthoredIdentityFailureArea =
  | "cursor-symbol"
  | "definition"
  | "formula"
  | "references"
  | "prepare-rename"
  | "rename";

export interface AuthoredIdentityFailure {
  readonly area: AuthoredIdentityFailureArea;
  readonly basis: string;
}

export interface AuthoredSourceAnchor {
  readonly fileId: string;
  readonly needle: string;
  readonly occurrence?: number;
  readonly selection?: {
    readonly length: number;
    readonly offset: number;
  };
}

export interface AuthoredLocationExpectation {
  /** Exhaustive source locations authorized by fixture schema 2. */
  readonly allowed?: readonly AuthoredSourceAnchor[];
  /**
   * Explicit entity-surface authorization when result availability alone is
   * insufficient (for example, go-to-definition at the declaration itself).
   */
  readonly authorization?: "authorized" | "refused";
  readonly excluded: readonly AuthoredSourceAnchor[];
  readonly minimum: number;
  readonly required: readonly AuthoredSourceAnchor[];
  readonly status: "available" | "unavailable";
}

export interface AuthoredScientificBatch {
  readonly createdAt: string;
  readonly frozenAt?: string;
  readonly id: string;
  readonly taskCardDigest: string;
  readonly reviewPolicyVersion: number;
  readonly seal?: string;
  readonly split: AuthoredSplit;
}

export interface AuthoredScientificSnapshot {
  readonly documents: readonly CorpusDocument[];
  readonly id: string;
}

export interface AuthoredScientificReview {
  readonly correctionSummary: readonly string[];
  readonly criticId: string;
  readonly finalDigest: string;
  readonly frozenAt?: string;
  readonly mainReviewer: string;
  readonly reviewedAt: string;
  readonly semanticReviewDigest: string;
  readonly status: "approved" | "corrected";
}

export interface AuthoredScientificScenario {
  readonly field: AuthoredArea;
  readonly genre: string;
  readonly id: string;
  readonly lawIds: readonly string[];
  readonly provenance: {
    readonly authorId: string;
    readonly engineBlind: true;
    readonly independenceGroup: string;
    readonly rawDigest: string;
    readonly taskCardDigest: string;
  };
  readonly review: AuthoredScientificReview;
  readonly snapshots: readonly AuthoredScientificSnapshot[];
  readonly variationTags: readonly string[];
}

export interface AuthoredRelationExpectation {
  readonly anchor: AuthoredSourceAnchor;
  readonly relationId: string;
  readonly roles: readonly ObservedRole[];
  readonly sourceGrounded: boolean;
}

export interface AuthoredDiagnosticExpectation {
  readonly anchor: AuthoredSourceAnchor;
  readonly code: string;
}

export interface AuthoredScientificProbe {
  readonly cursor: {
    readonly edge?: "after" | "before";
    readonly fileId: string;
    readonly needle: string;
    readonly occurrence?: number;
    readonly offset?: number;
    readonly snapshotId: string;
  };
  readonly expected: {
    readonly authoringContext?: StableMathAuthoringContext;
    readonly cursorOccurrence?: AuthoredSourceAnchor | null;
    readonly decision: ScientificDecision;
    readonly diagnostics: {
      readonly excludedCodes: readonly string[];
      readonly maximum: number;
      readonly required: readonly AuthoredDiagnosticExpectation[];
    };
    readonly excludedRelationIds: readonly string[];
    readonly formulaDecision?: {
      readonly anchor: AuthoredSourceAnchor;
      readonly status: MathAuthoringDisposition;
    } | null;
    readonly proofGrounded: boolean;
    readonly navigation: {
      readonly definition: AuthoredLocationExpectation;
      readonly prepareRename: {
        readonly authorization?: "authorized" | "refused";
        readonly placeholder?: string;
        readonly range?: AuthoredSourceAnchor;
        readonly status: "available" | "unavailable";
      };
      readonly references: AuthoredLocationExpectation;
      readonly rename: AuthoredLocationExpectation & {
        readonly expectedText?: string;
        readonly newName?: string;
        readonly replacementText?: string;
        readonly safety?: string;
      };
    };
    readonly relations: readonly AuthoredRelationExpectation[];
    readonly symbol?: string;
  };
  readonly family: DocumentReasoningFamily;
  readonly id: string;
  readonly kind: "primary" | "supplemental";
  readonly scenarioId: string;
}

export interface AuthoredScientificFixture {
  readonly batch: AuthoredScientificBatch;
  readonly probes: readonly AuthoredScientificProbe[];
  readonly scenarios: readonly AuthoredScientificScenario[];
  readonly schemaVersion: 1 | 2;
}

export interface AuthoredLawCatalogEntry {
  readonly field: string;
  readonly lawId: string;
  readonly roles: readonly { readonly id: string; readonly variadic: boolean }[];
}

export interface ObservedLocation {
  readonly fileId: string;
  readonly path: string;
  readonly range: SourceRange;
}

export type AuthoredObservedSurfaceAuthorization =
  | {
      readonly entityId: Extract<
        EntitySurfaceAuthorization,
        { readonly status: "authorized" }
      >["entityId"];
      readonly focusOccurrenceId: Extract<
        EntitySurfaceAuthorization,
        { readonly status: "authorized" }
      >["focusOccurrenceId"];
      readonly status: "authorized";
    }
  | {
      readonly refusalKind: Extract<
        EntitySurfaceAuthorization,
        { readonly status: "refused" }
      >["reason"]["kind"];
      readonly status: "refused";
    };

export interface AuthoredScientificObservation {
  readonly authoringContext?: MathAuthoringContext;
  readonly caseId: string;
  /** Exact cursor identity from semanticView; required by current fresh schema 3. */
  readonly cursorSurfaceIdentity?: {
    readonly entityId: EntityId | null;
    readonly location: ObservedLocation;
    readonly occurrenceId: SourceOccurrenceId;
  } | null;
  readonly decision: ScientificDecision;
  readonly definitions: readonly ObservedLocation[];
  readonly diagnostics: readonly {
    readonly code: string;
    readonly fileId: string;
    readonly range: SourceRange;
    readonly severity: "error" | "hint" | "warning";
  }[];
  /** Required by authored fixture schema 2; derived from authoringContext. */
  readonly formulaDecision?: {
    readonly location: ObservedLocation | null;
    readonly status: MathAuthoringDisposition;
  };
  /** Present in protocol 16+ reports; omitted from preserved historical observations. */
  readonly interpretations?: MathInterpretationSetInfo;
  readonly prepareRename: {
    readonly placeholder?: string;
    readonly range?: SourceRange;
  };
  readonly proofGrounded: boolean;
  readonly references: readonly ObservedLocation[];
  readonly relations: readonly {
    readonly fileId: string;
    readonly relationId: string;
    readonly range: SourceRange;
    readonly formulaRange?: SourceRange;
    readonly roles: readonly ObservedRole[];
    readonly sourceGrounded: boolean;
  }[];
  readonly renameEdits: readonly {
    readonly expectedText: string;
    readonly fileId: string;
    readonly path: string;
    readonly range: SourceRange;
    readonly replacementText: string;
  }[];
  readonly renameSafety?: string;
  readonly symbol: string | null;
  readonly symbolLocation?: ObservedLocation;
  /** Required for current fresh schema 3; optional for retained legacy reports. */
  readonly surfaceAuthorizations?: {
    readonly definition: AuthoredObservedSurfaceAuthorization;
    readonly prepareRename: AuthoredObservedSurfaceAuthorization;
    readonly references: AuthoredObservedSurfaceAuthorization;
    readonly rename: AuthoredObservedSurfaceAuthorization;
  };
}

export interface AuthoredScientificSurfaceResults {
  readonly definition: QueryResult;
  readonly diagnostics: QueryResult;
  readonly prepareRename: QueryResult;
  readonly references: QueryResult;
  readonly rename: QueryResult;
  readonly relationViews?: readonly {
    readonly fileId: string;
    readonly result: QueryResult;
  }[];
  readonly semanticView: QueryResult;
}

export interface AuthoredScientificScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly passed: number;
  readonly risk: {
    readonly falseConflict: number;
    readonly falseEstablishment: number;
    readonly missedCoverage: number;
    readonly navigationOrIdentity: number;
    readonly total: number;
  };
}

export type AuthoredFalseEstablishmentCause =
  | "decision"
  | "excluded-relation"
  | "formula-decision"
  | "proof-grounding"
  | "relation-grounding";

export interface AuthoredFalseEstablishmentCase {
  readonly caseId: string;
  readonly causes: readonly AuthoredFalseEstablishmentCause[];
  readonly sourceGrounded: boolean;
}

export interface AuthoredTrancheSummary {
  readonly decisions: Readonly<Record<ScientificDecision, number>>;
  readonly developmentCases: number;
  readonly fields: Readonly<Record<AuthoredArea, number>>;
  readonly holdoutCases: number;
  readonly holdoutFamilies: Readonly<Record<DocumentReasoningFamily, number>>;
  readonly laws: number;
}

export function parseAuthoredScientificFixture(
  value: unknown,
): AuthoredScientificFixture {
  const root = record(value, "fixture");
  exact(root, ["schemaVersion", "batch", "scenarios", "probes"], "fixture");
  const parsedSchemaVersion = integer(
    root.schemaVersion,
    "fixture.schemaVersion",
  );
  if (parsedSchemaVersion !== 1 && parsedSchemaVersion !== 2) {
    throw new Error("fixture.schemaVersion: must be 1 or 2");
  }
  const schemaVersion = parsedSchemaVersion;
  const batch = parseBatch(root.batch);
  const scenarios = array(root.scenarios, "fixture.scenarios").map(parseScenario);
  const probes = array(root.probes, "fixture.probes").map((probe, index) =>
    parseProbe(probe, index, schemaVersion)
  );
  unique(scenarios.map((item) => item.id), "fixture.scenarios.id");
  unique(probes.map((item) => item.id), "fixture.probes.id");
  const scenarioById = new Map(scenarios.map((item) => [item.id, item]));
  for (const probe of probes) {
    const scenario = scenarioById.get(probe.scenarioId);
    if (!scenario) throw new Error(`${probe.id}: unknown scenario ${probe.scenarioId}`);
    validateProbe(probe, scenario, schemaVersion);
  }
  for (const scenario of scenarios) {
    const caseProbes = probes.filter((probe) => probe.scenarioId === scenario.id);
    if (caseProbes.filter((probe) => probe.kind === "primary").length !== 1) {
      throw new Error(`${scenario.id}: scenario requires exactly one primary probe`);
    }
    if (scenario.provenance.authorId === scenario.review.criticId) {
      throw new Error(`${scenario.id}: critic must be independent from author`);
    }
    if (scenario.review.finalDigest !== scenario.review.semanticReviewDigest) {
      throw new Error(`${scenario.id}: semantic review must approve the final digest`);
    }
    const corrected = scenario.review.status === "corrected";
    if (corrected !== (scenario.review.correctionSummary.length > 0)) {
      throw new Error(`${scenario.id}: correction status and summary disagree`);
    }
    if (corrected && scenario.provenance.rawDigest === scenario.review.finalDigest) {
      throw new Error(`${scenario.id}: corrected source must differ from raw output`);
    }
    if (batch.split === "holdout" && !scenario.review.frozenAt) {
      throw new Error(`${scenario.id}: holdout review must be frozen`);
    }
    if (batch.split === "development" && scenario.review.frozenAt) {
      throw new Error(`${scenario.id}: development scenario must remain editable`);
    }
  }
  if (batch.split === "holdout") {
    if (!batch.frozenAt || !batch.seal) {
      throw new Error("fixture.batch: holdout requires frozenAt and seal");
    }
  } else if (batch.frozenAt || batch.seal) {
    throw new Error("fixture.batch: development must remain editable and unsealed");
  }
  return { batch, probes, scenarios, schemaVersion };
}

export function validateAuthoredScientificTranche(
  development: AuthoredScientificFixture,
  holdout: AuthoredScientificFixture,
  lawCatalog: readonly AuthoredLawCatalogEntry[],
  priorityFields: readonly string[],
): AuthoredTrancheSummary {
  requireSplit(development, "development");
  requireSplit(holdout, "holdout");
  if (development.scenarios.length !== 96) {
    throw new Error("development fixture requires exactly 96 authored cases");
  }
  if (holdout.scenarios.length !== 48) {
    throw new Error("holdout fixture requires exactly 48 authored cases");
  }
  const allScenarios = [...development.scenarios, ...holdout.scenarios];
  const primaryDevelopment = primaryProbes(development);
  const primaryHoldout = primaryProbes(holdout);
  const allPrimary = [...primaryDevelopment, ...primaryHoldout];
  unique(allScenarios.map((item) => item.id), "tranche scenario ids");
  unique(
    [...development.probes, ...holdout.probes].map((item) => item.id),
    "tranche probe ids",
  );
  rejectLineageLeakage(development, holdout);
  validateAreaAllocation(development);
  validateAreaAllocation(holdout);

  const decisions = countBy(
    allPrimary.map((probe) =>
      probe.expected.formulaDecision?.status ?? probe.expected.decision
    ).filter((decision): decision is ScientificDecision =>
      decision !== "conventional" && decision !== "engine-limited"
    ),
    ["established", "partial", "ambiguous", "conflicting", "unsupported"] as const,
  );
  for (const decision of Object.keys(AUTHORED_DECISION_TARGET) as ScientificDecision[]) {
    if (decisions[decision] === 0) {
      throw new Error(`${decision}: authored tranche must contain reviewed cases`);
    }
  }

  const holdoutFamilies = countBy(
    primaryHoldout.map((probe) => probe.family),
    DOCUMENT_REASONING_FAMILIES,
  );
  for (const family of DOCUMENT_REASONING_FAMILIES) {
    if (holdoutFamilies[family] !== 8) {
      throw new Error(`${family}: holdout requires exactly 8 cases`);
    }
  }

  unique(lawCatalog.map((item) => item.lawId), "law catalog ids");
  for (const law of lawCatalog) unique(law.roles.map((role) => role.id), `${law.lawId}.roles`);
  const knownLaws = new Set(lawCatalog.map((item) => item.lawId));
  const lawsById = new Map(lawCatalog.map((law) => [law.lawId, law]));
  for (const scenario of allScenarios) {
    for (const lawId of scenario.lawIds) {
      if (!knownLaws.has(lawId)) throw new Error(`${scenario.id}: unknown law ${lawId}`);
    }
  }
  for (const fixture of [development, holdout]) {
    for (const probe of fixture.probes) {
      const scenario = authoredScenarioFor(fixture, probe);
      for (const relation of probe.expected.relations) {
        if (!knownLaws.has(relation.relationId)) {
          throw new Error(`${probe.id}: unknown expected relation ${relation.relationId}`);
        }
        if (!scenario.lawIds.includes(relation.relationId)) {
          throw new Error(`${probe.id}: expected relation is absent from scenario law coverage`);
        }
        const law = lawsById.get(relation.relationId)!;
        const roleIds = new Set(law.roles.map((role) => role.id));
        if (relation.roles.some((role) => !roleIds.has(role.role))) {
          throw new Error(`${probe.id}: ${relation.relationId} has an unknown authored role`);
        }
        for (const role of law.roles) {
          const count = relation.roles.filter((candidate) => candidate.role === role.id).length;
          if (role.variadic ? count === 0 : count !== 1) {
            throw new Error(`${probe.id}: ${relation.relationId} requires exact authored roles`);
          }
        }
      }
      for (const relationId of probe.expected.excludedRelationIds) {
        if (!knownLaws.has(relationId)) {
          throw new Error(`${probe.id}: unknown excluded relation ${relationId}`);
        }
      }
    }
  }
  for (const law of lawCatalog) {
    const matches = allScenarios.filter((scenario) => scenario.lawIds.includes(law.lawId));
    if (!matches.length) throw new Error(`${law.lawId}: missing authored coverage`);
    if (
      priorityFields.includes(law.field) &&
      new Set(matches.map((scenario) => scenario.genre)).size < 2
    ) {
      throw new Error(`${law.lawId}: priority law requires two document genres`);
    }
  }
  return {
    decisions,
    developmentCases: development.scenarios.length,
    fields: Object.fromEntries(
      (Object.keys(AUTHORED_AREA_ALLOCATION) as AuthoredArea[]).map((field) => [
        field,
        allScenarios.filter((item) => item.field === field).length,
      ]),
    ) as Record<AuthoredArea, number>,
    holdoutCases: holdout.scenarios.length,
    holdoutFamilies,
    laws: lawCatalog.length,
  };
}

export function scoreAuthoredScientificFixture(
  fixture: AuthoredScientificFixture,
  observations: readonly AuthoredScientificObservation[],
): AuthoredScientificScorecard {
  const failures: string[] = [];
  const failedCases = new Set<string>();
  const expectedIds = new Set(fixture.probes.map((probe) => probe.id));
  const byId = new Map<string, AuthoredScientificObservation>();
  let invalidObservationSet = false;
  for (const observation of observations) {
    if (!expectedIds.has(observation.caseId)) {
      failures.push(`${observation.caseId}: unexpected observation`);
      invalidObservationSet = true;
    } else if (byId.has(observation.caseId)) {
      failures.push(`${observation.caseId}: duplicate observation`);
      invalidObservationSet = true;
    } else {
      byId.set(observation.caseId, observation);
    }
  }
  let falseConflict = 0;
  let falseEstablishment = 0;
  let missedCoverage = 0;
  let navigationOrIdentity = 0;
  const falseEstablishmentById = new Map(
    authoredFalseEstablishmentCases(fixture, observations).map((item) => [
      item.caseId,
      item,
    ]),
  );
  for (const probe of fixture.probes) {
    const observed = byId.get(probe.id);
    if (!observed) {
      failures.push(`${probe.id}: missing observation`);
      failedCases.add(probe.id);
      missedCoverage += 1;
      continue;
    }
    const caseFailures: string[] = [];
    let caseFalseConflict = false;
    const falseEstablishmentCauses = new Set(
      falseEstablishmentById.get(probe.id)?.causes,
    );
    const caseFalseEstablishment = falseEstablishmentCauses.size > 0;
    let caseMissedCoverage = false;
    let caseNavigation = false;
    if (observed.decision !== probe.expected.decision) {
      caseFailures.push(`decision ${observed.decision}; expected ${probe.expected.decision}`);
      caseFalseConflict =
        observed.decision === "conflicting" && probe.expected.decision !== "conflicting";
      caseMissedCoverage =
        !falseEstablishmentCauses.has("decision") && !caseFalseConflict;
    }
    const expectedFormula = probe.expected.formulaDecision;
    if (expectedFormula === null) {
      if (
        observed.formulaDecision === undefined ||
        observed.formulaDecision.location !== null
      ) {
        caseFailures.push("selected formula must be absent");
        caseMissedCoverage = true;
      }
    } else if (expectedFormula) {
      const actualFormula = observed.formulaDecision;
      if (!actualFormula) {
        caseFailures.push("selected formula decision is missing");
        caseMissedCoverage = true;
      } else if (actualFormula.status !== expectedFormula.status) {
        caseFailures.push(
          `formula decision ${actualFormula.status}; expected ${expectedFormula.status}`,
        );
        const formulaFalseConflict =
          actualFormula.status === "conflicting" &&
          expectedFormula.status !== "conflicting";
        caseFalseConflict ||= formulaFalseConflict;
        if (
          !falseEstablishmentCauses.has("formula-decision") &&
          !formulaFalseConflict
        ) {
          caseMissedCoverage = true;
        }
      }
    }
    if (observed.proofGrounded !== probe.expected.proofGrounded) {
      caseFailures.push(
        `proof grounding ${observed.proofGrounded}; expected ${probe.expected.proofGrounded}`,
      );
      if (!observed.proofGrounded) caseMissedCoverage = true;
    }
    for (const expected of probe.expected.relations) {
      const expectedAnchor = resolveAuthoredAnchor(
        authoredSnapshotFor(authoredScenarioFor(fixture, probe), probe),
        expected.anchor,
      );
      const expectedDocument = authoredSnapshotFor(
        authoredScenarioFor(fixture, probe),
        probe,
      ).documents.find((document) => document.fileId === expectedAnchor.fileId);
      const relation = observed.relations.find(
        (item) =>
          item.relationId === expected.relationId &&
          item.fileId === expectedAnchor.fileId &&
          expectedDocument !== undefined &&
          authoredRelationRangeMatches(
            expectedDocument.content,
            item.range,
            expectedAnchor.range,
            item.formulaRange,
          ) &&
          roleInstancesMatch(item.roles, expected.roles, undefined),
      );
      if (!relation) {
        caseFailures.push(`missing relation ${expected.relationId} at ${expected.anchor.fileId}:${expected.anchor.needle}`);
        caseMissedCoverage = true;
        continue;
      }
      if (relation.sourceGrounded !== expected.sourceGrounded) {
        caseFailures.push(
          `${expected.relationId}: source grounding ${relation.sourceGrounded}; expected ${expected.sourceGrounded}`,
        );
        if (!relation.sourceGrounded) caseMissedCoverage = true;
      }
    }
    for (const relationId of probe.expected.excludedRelationIds) {
      if (observed.relations.some((relation) => relation.relationId === relationId)) {
        caseFailures.push(`leaked relation ${relationId}`);
      }
    }
    const scenario = authoredScenarioFor(fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
    const identityFailures = authoredProbeIdentityFailures(
      fixture,
      probe,
      observed,
    );
    caseFailures.push(...identityFailures.map((failure) => failure.basis));
    caseNavigation = identityFailures.length > 0;
    const problems = observed.diagnostics.filter(
      (item) => item.severity === "error" || item.severity === "warning",
    );
    if (problems.length > probe.expected.diagnostics.maximum) {
      caseFailures.push(
        `problems ${problems.length}; expected at most ${probe.expected.diagnostics.maximum}`,
      );
    }
    for (const expected of probe.expected.diagnostics.required) {
      const anchor = resolveAuthoredAnchor(snapshot, expected.anchor);
      if (
        !problems.some(
          (problem) =>
            problem.code === expected.code &&
            problem.fileId === anchor.fileId &&
            sameRange(problem.range, anchor.range),
        )
      ) {
        caseFailures.push(`missing problem ${expected.code}`);
        caseMissedCoverage = true;
      }
    }
    for (const code of probe.expected.diagnostics.excludedCodes) {
      if (problems.some((problem) => problem.code === code)) {
        caseFailures.push(`unexpected problem ${code}`);
        caseFalseConflict = true;
      }
    }
    falseConflict += Number(caseFalseConflict);
    falseEstablishment += Number(caseFalseEstablishment);
    missedCoverage += Number(caseMissedCoverage);
    navigationOrIdentity += Number(caseNavigation);
    if (caseFailures.length) {
      failedCases.add(probe.id);
      failures.push(`${probe.id}: ${caseFailures.join("; ")}`);
    }
  }
  return {
    cases: fixture.probes.length,
    failures,
    passed: invalidObservationSet ? 0 : fixture.probes.length - failedCases.size,
    risk: {
      falseConflict,
      falseEstablishment,
      missedCoverage,
      navigationOrIdentity,
      total:
        falseConflict * 12 +
        falseEstablishment * 12 +
        navigationOrIdentity * 10 +
        missedCoverage * 2,
    },
  };
}

export function authoredFalseEstablishmentCases(
  fixture: AuthoredScientificFixture,
  observations: readonly AuthoredScientificObservation[],
): readonly AuthoredFalseEstablishmentCase[] {
  const expectedIds = new Set(fixture.probes.map((probe) => probe.id));
  const byId = new Map<string, AuthoredScientificObservation>();
  for (const observation of observations) {
    if (expectedIds.has(observation.caseId) && !byId.has(observation.caseId)) {
      byId.set(observation.caseId, observation);
    }
  }
  const cases: AuthoredFalseEstablishmentCase[] = [];
  for (const probe of fixture.probes) {
    const observed = byId.get(probe.id);
    if (!observed) continue;
    const causes = new Set<AuthoredFalseEstablishmentCause>();
    const grounding: boolean[] = [];
    if (
      observed.decision === "established" &&
      probe.expected.decision !== "established"
    ) {
      causes.add("decision");
      grounding.push(observed.proofGrounded);
    }
    if (
      probe.expected.formulaDecision &&
      observed.formulaDecision?.status === "established" &&
      probe.expected.formulaDecision.status !== "established"
    ) {
      causes.add("formula-decision");
      const authority = (observed.authoringContext?.interpretations ??
        observed.interpretations)?.hypotheses.filter(
          hypothesisIsMathematicalAuthority,
        ) ?? [];
      grounding.push(
        authority.length > 0 && authority.every((hypothesis) =>
          hypothesis.evidence.length > 0 &&
          hypothesis.evidence.every((evidence) =>
            evidence.sourceAnchors.length > 0
          )
        ),
      );
    }
    if (observed.proofGrounded && !probe.expected.proofGrounded) {
      causes.add("proof-grounding");
      grounding.push(true);
    }
    for (const expected of probe.expected.relations) {
      const snapshot = authoredSnapshotFor(
        authoredScenarioFor(fixture, probe),
        probe,
      );
      const expectedAnchor = resolveAuthoredAnchor(snapshot, expected.anchor);
      const expectedDocument = snapshot.documents.find(
        (document) => document.fileId === expectedAnchor.fileId,
      );
      const relation = observed.relations.find(
        (item) =>
          item.relationId === expected.relationId &&
          item.fileId === expectedAnchor.fileId &&
          expectedDocument !== undefined &&
          authoredRelationRangeMatches(
            expectedDocument.content,
            item.range,
            expectedAnchor.range,
            item.formulaRange,
          ) &&
          roleInstancesMatch(item.roles, expected.roles, undefined),
      );
      if (
        relation?.sourceGrounded &&
        relation.sourceGrounded !== expected.sourceGrounded
      ) {
        causes.add("relation-grounding");
        grounding.push(true);
      }
    }
    const leakedRelations = observed.relations.filter((relation) =>
      probe.expected.excludedRelationIds.includes(relation.relationId),
    );
    const interpretations = observed.authoringContext?.interpretations ??
      observed.interpretations;
    const falselyEstablishedRelations = interpretations === undefined
      ? leakedRelations
      : leakedRelations.filter((relation) =>
        interpretations.hypotheses.some((hypothesis) =>
          hypothesis.relation?.relationId === relation.relationId &&
          hypothesisIsMathematicalAuthority(hypothesis)
        )
      );
    if (falselyEstablishedRelations.length > 0) {
      causes.add("excluded-relation");
      grounding.push(
        falselyEstablishedRelations.every((relation) => relation.sourceGrounded),
      );
    }
    if (causes.size > 0) {
      cases.push({
        caseId: probe.id,
        causes: [...causes],
        sourceGrounded: grounding.every(Boolean),
      });
    }
  }
  return cases;
}

export function observeAuthoredScientificProbe(
  probe: AuthoredScientificProbe,
  results: AuthoredScientificSurfaceResults,
): AuthoredScientificObservation {
  if (results.semanticView.value.kind !== "semanticView") {
    throw new Error(`${probe.id}: semanticView result is missing`);
  }
  if (
    results.definition.value.kind !== "locations" ||
    results.references.value.kind !== "locations" ||
    results.prepareRename.value.kind !== "renamePreparation" ||
    results.rename.value.kind !== "editProposal" ||
    results.diagnostics.value.kind !== "diagnostics"
  ) {
    throw new Error(`${probe.id}: public surface results are incomplete`);
  }
  const view = results.semanticView.value.view;
  const relations = [
    ...observeAuthoredRelations(probe.cursor.fileId, view.context.relations),
    ...(results.relationViews ?? []).flatMap(({ fileId, result }) => {
      if (result.value.kind !== "semanticView") {
        throw new Error(`${probe.id}: relation source semanticView is missing`);
      }
      return observeAuthoredRelations(fileId, result.value.view.context.relations);
    }),
  ].filter(
    (relation, index, all) =>
      all.findIndex(
        (candidate) =>
          candidate.fileId === relation.fileId &&
          candidate.relationId === relation.relationId &&
          candidate.range.startOffset === relation.range.startOffset &&
          candidate.range.endOffset === relation.range.endOffset,
      ) === index,
  );
  const proofEvidence = view.decision.reasons
    .filter((reason) => reason.kind === "proof" || reason.kind === "source-conflict")
    .flatMap((reason) => reason.evidence);
  return {
    authoringContext: view.authoringContext,
    caseId: probe.id,
    cursorSurfaceIdentity: view.symbol
      ? {
          entityId: view.symbol.entityId ?? null,
          location: view.symbol.location,
          occurrenceId: view.symbol.occurrenceId,
        }
      : null,
    decision: view.decision.status,
    definitions: results.definition.value.locations,
    diagnostics: results.diagnostics.value.diagnostics.map((diagnostic) => ({
      code: diagnostic.code,
      fileId: probe.cursor.fileId,
      range: diagnostic.range,
      severity: diagnostic.severity,
    })),
    ...(probe.expected.formulaDecision !== undefined
      ? {
          formulaDecision: {
            location: view.authoringContext.formula?.location ?? null,
            status: view.authoringContext.disposition,
          },
        }
      : {}),
    interpretations: view.authoringContext.interpretations,
    prepareRename: {
      ...(results.prepareRename.value.placeholder
        ? { placeholder: results.prepareRename.value.placeholder }
        : {}),
      ...(results.prepareRename.value.range
        ? { range: results.prepareRename.value.range }
        : {}),
    },
    proofGrounded:
      view.symbol?.entityId !== undefined &&
      proofEvidence.length > 0 &&
      proofEvidence.every((evidence) => evidence.sourceRanges.length > 0),
    references: results.references.value.locations,
    relations,
    renameEdits: (results.rename.value.proposal?.files ?? []).flatMap((file) =>
      file.edits.map((edit) => ({
        expectedText: edit.expectedText,
        fileId: file.fileId,
        path: file.path,
        range: edit.range,
        replacementText: edit.replacementText,
      })),
    ),
    ...(results.rename.value.proposal
      ? { renameSafety: results.rename.value.proposal.safety }
      : {}),
    symbol: view.symbol?.symbol ?? null,
    ...(view.symbol ? { symbolLocation: view.symbol.location } : {}),
    surfaceAuthorizations: {
      definition: observeSurfaceAuthorization(
        results.definition.value.authorization,
      ),
      prepareRename: observeSurfaceAuthorization(
        results.prepareRename.value.authorization,
      ),
      references: observeSurfaceAuthorization(
        results.references.value.authorization,
      ),
      rename: observeSurfaceAuthorization(results.rename.value.authorization),
    },
  };
}

function observeSurfaceAuthorization(
  authorization: EntitySurfaceAuthorization,
): AuthoredObservedSurfaceAuthorization {
  return authorization.status === "authorized"
    ? {
        entityId: authorization.entityId,
        focusOccurrenceId: authorization.focusOccurrenceId,
        status: "authorized",
      }
    : {
        refusalKind: authorization.reason.kind,
        status: "refused",
      };
}

export function observeAuthoredRelations(
  fileId: string,
  relations: readonly RelationInfo[],
): AuthoredScientificObservation["relations"] {
  return relations.map((relation) => {
    const formulaRange = relation.evidence.find(
      (evidence) => evidence.ruleId === "semantic-law-unification",
    )?.sourceRanges[0];
    return {
      fileId,
      relationId: relation.relationId,
      range: relation.range,
      ...(formulaRange && !sameRange(formulaRange, relation.range)
        ? { formulaRange }
        : {}),
      roles: relation.roles.map((role) => ({
        ...(role.conceptId ? { conceptId: role.conceptId } : {}),
        role: role.role,
        symbol: role.symbol,
      })),
      sourceGrounded:
        relation.evidence.length > 0 &&
        relation.evidence.every((evidence) => evidence.sourceRanges.length > 0),
    };
  });
}

export function authoredScenarioReviewPayload(
  fixture: AuthoredScientificFixture,
  scenarioId: string,
): string {
  const scenario = fixture.scenarios.find((item) => item.id === scenarioId);
  if (!scenario) throw new Error(`${scenarioId}: unknown scenario`);
  const { review: _review, ...reviewedScenario } = scenario;
  return stableJson({
    probes: fixture.probes.filter((probe) => probe.scenarioId === scenarioId),
    scenario: reviewedScenario,
  });
}

export function authoredFixtureSealPayload(fixture: AuthoredScientificFixture): string {
  const { seal: _seal, ...batch } = fixture.batch;
  return stableJson({ ...fixture, batch });
}

function stableJson(value: unknown): string {
  return JSON.stringify(sortJsonValue(value));
}

function sortJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortJsonValue);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => [key, sortJsonValue(item)]),
  );
}

function parseBatch(value: unknown): AuthoredScientificBatch {
  const item = record(value, "fixture.batch");
  exact(
    item,
    [
      "id", "split", "createdAt", "taskCardDigest", "reviewPolicyVersion", "frozenAt", "seal",
    ],
    "fixture.batch",
    ["frozenAt", "seal"],
  );
  return {
    createdAt: date(item.createdAt, "fixture.batch.createdAt"),
    ...(item.frozenAt === undefined ? {} : { frozenAt: timestamp(item.frozenAt, "fixture.batch.frozenAt") }),
    id: text(item.id, "fixture.batch.id"),
    reviewPolicyVersion: positiveInteger(item.reviewPolicyVersion, "fixture.batch.reviewPolicyVersion"),
    ...(item.seal === undefined ? {} : { seal: digest(item.seal, "fixture.batch.seal") }),
    split: oneOf(item.split, ["development", "holdout"] as const, "fixture.batch.split"),
    taskCardDigest: digest(item.taskCardDigest, "fixture.batch.taskCardDigest"),
  };
}

function parseScenario(value: unknown, index: number): AuthoredScientificScenario {
  const path = `fixture.scenarios[${index}]`;
  const item = record(value, path);
  exact(
    item,
    ["id", "field", "genre", "lawIds", "snapshots", "variationTags", "provenance", "review"],
    path,
  );
  const snapshots = array(item.snapshots, `${path}.snapshots`).map((value, snapshotIndex) =>
    parseSnapshot(value, `${path}.snapshots[${snapshotIndex}]`),
  );
  if (!snapshots.length) throw new Error(`${path}.snapshots: must not be empty`);
  unique(snapshots.map((snapshot) => snapshot.id), `${path}.snapshots.id`);
  return {
    field: oneOf(
      item.field,
      Object.keys(AUTHORED_AREA_ALLOCATION) as AuthoredArea[],
      `${path}.field`,
    ),
    genre: text(item.genre, `${path}.genre`),
    id: text(item.id, `${path}.id`),
    lawIds: strings(item.lawIds, `${path}.lawIds`),
    provenance: parseProvenance(item.provenance, `${path}.provenance`),
    review: parseReview(item.review, `${path}.review`),
    snapshots,
    variationTags: strings(item.variationTags, `${path}.variationTags`, 2),
  };
}

function parseSnapshot(value: unknown, path: string): AuthoredScientificSnapshot {
  const item = record(value, path);
  exact(item, ["id", "documents"], path);
  const documents = array(item.documents, `${path}.documents`).map((value, index) => {
    const documentPath = `${path}.documents[${index}]`;
    const document = record(value, documentPath);
    exact(document, ["fileId", "path", "content"], documentPath);
    return {
      content: text(document.content, `${documentPath}.content`),
      fileId: text(document.fileId, `${documentPath}.fileId`),
      path: text(document.path, `${documentPath}.path`),
    };
  });
  if (!documents.length) throw new Error(`${path}.documents: must not be empty`);
  unique(documents.map((document) => document.fileId), `${path}.documents.fileId`);
  unique(documents.map((document) => document.path), `${path}.documents.path`);
  return { documents, id: text(item.id, `${path}.id`) };
}

function parseProvenance(
  value: unknown,
  path: string,
): AuthoredScientificScenario["provenance"] {
  const item = record(value, path);
  exact(
    item,
    ["authorId", "engineBlind", "independenceGroup", "taskCardDigest", "rawDigest"],
    path,
  );
  if (item.engineBlind !== true) throw new Error(`${path}.engineBlind: must be true`);
  return {
    authorId: text(item.authorId, `${path}.authorId`),
    engineBlind: true,
    independenceGroup: text(item.independenceGroup, `${path}.independenceGroup`),
    rawDigest: digest(item.rawDigest, `${path}.rawDigest`),
    taskCardDigest: digest(item.taskCardDigest, `${path}.taskCardDigest`),
  };
}

function parseReview(value: unknown, path: string): AuthoredScientificReview {
  const item = record(value, path);
  exact(
    item,
    [
      "criticId", "mainReviewer", "status", "correctionSummary", "finalDigest",
      "semanticReviewDigest", "reviewedAt", "frozenAt",
    ],
    path,
    ["frozenAt"],
  );
  return {
    correctionSummary: strings(item.correctionSummary, `${path}.correctionSummary`),
    criticId: text(item.criticId, `${path}.criticId`),
    finalDigest: digest(item.finalDigest, `${path}.finalDigest`),
    ...(item.frozenAt === undefined ? {} : { frozenAt: timestamp(item.frozenAt, `${path}.frozenAt`) }),
    mainReviewer: text(item.mainReviewer, `${path}.mainReviewer`),
    reviewedAt: date(item.reviewedAt, `${path}.reviewedAt`),
    semanticReviewDigest: digest(item.semanticReviewDigest, `${path}.semanticReviewDigest`),
    status: oneOf(item.status, ["approved", "corrected"] as const, `${path}.status`),
  };
}

function parseProbe(
  value: unknown,
  index: number,
  schemaVersion: AuthoredScientificFixture["schemaVersion"],
): AuthoredScientificProbe {
  const path = `fixture.probes[${index}]`;
  const item = record(value, path);
  exact(
    item,
    [
      "id", "scenarioId", "kind", "family", "cursor", "expected",
    ],
    path,
  );
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(
    cursor,
    ["snapshotId", "fileId", "needle", "occurrence", "edge", "offset"],
    `${path}.cursor`,
    ["occurrence", "edge", "offset"],
  );
  if ((cursor.edge === undefined) === (cursor.offset === undefined)) {
    throw new Error(`${path}.cursor: exactly one of edge or offset is required`);
  }
  const needle = text(cursor.needle, `${path}.cursor.needle`);
  const offset =
    cursor.offset === undefined
      ? undefined
      : integer(cursor.offset, `${path}.cursor.offset`);
  if (offset !== undefined && offset > needle.length) {
    throw new Error(`${path}.cursor.offset: must fall within the cursor needle`);
  }
  const expected = parseExpected(
    item.expected,
    `${path}.expected`,
    schemaVersion,
  );
  return {
    cursor: {
      ...(cursor.edge === undefined
        ? {}
        : { edge: oneOf(cursor.edge, ["after", "before"] as const, `${path}.cursor.edge`) }),
      fileId: text(cursor.fileId, `${path}.cursor.fileId`),
      needle,
      ...(cursor.occurrence === undefined
        ? {}
        : { occurrence: integer(cursor.occurrence, `${path}.cursor.occurrence`) }),
      ...(offset === undefined ? {} : { offset }),
      snapshotId: text(cursor.snapshotId, `${path}.cursor.snapshotId`),
    },
    expected,
    family: oneOf(item.family, DOCUMENT_REASONING_FAMILIES, `${path}.family`),
    id: text(item.id, `${path}.id`),
    kind: oneOf(item.kind, ["primary", "supplemental"] as const, `${path}.kind`),
    scenarioId: text(item.scenarioId, `${path}.scenarioId`),
  };
}

function parseExpected(
  value: unknown,
  path: string,
  schemaVersion: AuthoredScientificFixture["schemaVersion"],
): AuthoredScientificProbe["expected"] {
  const item = record(value, path);
  exact(
    item,
    [
      "authoringContext", "decision", "symbol", "cursorOccurrence", "formulaDecision", "proofGrounded", "relations", "excludedRelationIds",
      "navigation", "diagnostics",
    ],
    path,
    schemaVersion === 1
      ? ["authoringContext", "symbol", "cursorOccurrence", "formulaDecision"]
      : ["authoringContext", "symbol", "cursorOccurrence"],
  );
  if (schemaVersion === 1 && item.formulaDecision !== undefined) {
    throw new Error(`${path}.formulaDecision: unavailable in fixture schema 1`);
  }
  const navigation = record(item.navigation, `${path}.navigation`);
  exact(
    navigation,
    ["definition", "references", "prepareRename", "rename"],
    `${path}.navigation`,
  );
  const prepareRename = record(
    navigation.prepareRename,
    `${path}.navigation.prepareRename`,
  );
  exact(
    prepareRename,
    [
      "status",
      "range",
      "placeholder",
      ...(schemaVersion === 2 ? ["authorization"] : []),
    ],
    `${path}.navigation.prepareRename`,
    [
      "range",
      "placeholder",
      ...(schemaVersion === 2 ? ["authorization"] : []),
    ],
  );
  const rename = record(navigation.rename, `${path}.navigation.rename`);
  exact(
    rename,
    [
      "status",
      "minimum",
      "required",
      "excluded",
      ...(schemaVersion === 2 ? ["allowed", "authorization"] : []),
      "expectedText",
      "newName",
      "replacementText",
      "safety",
    ],
    `${path}.navigation.rename`,
    [
      ...(schemaVersion === 2 ? ["allowed", "authorization"] : []),
      "expectedText",
      "newName",
      "replacementText",
      "safety",
    ],
  );
  const diagnostics = record(item.diagnostics, `${path}.diagnostics`);
  exact(
    diagnostics,
    ["required", "excludedCodes", "maximum"],
    `${path}.diagnostics`,
  );
  const requiredDiagnostics = array(
    diagnostics.required,
    `${path}.diagnostics.required`,
  ).map((value, index) => {
    const diagnosticPath = `${path}.diagnostics.required[${index}]`;
    const diagnostic = record(value, diagnosticPath);
    exact(diagnostic, ["code", "anchor"], diagnosticPath);
    return {
      anchor: parseAnchor(diagnostic.anchor, `${diagnosticPath}.anchor`),
      code: text(diagnostic.code, `${diagnosticPath}.code`),
    };
  });
  const maximum = integer(diagnostics.maximum, `${path}.diagnostics.maximum`);
  if (maximum < requiredDiagnostics.length) {
    throw new Error(`${path}.diagnostics.maximum: smaller than required diagnostics`);
  }
  return {
    ...(item.authoringContext === undefined
      ? {}
      : {
          authoringContext: parseMathAuthoringContextExpectation(
            item.authoringContext,
            `${path}.authoringContext`,
          ),
        }),
    ...(item.cursorOccurrence === undefined
      ? {}
      : {
          cursorOccurrence:
            item.cursorOccurrence === null
              ? null
              : parseAnchor(item.cursorOccurrence, `${path}.cursorOccurrence`),
        }),
    decision: oneOf(
      item.decision,
      ["ambiguous", "conflicting", "established", "partial", "unsupported"] as const,
      `${path}.decision`,
    ),
    diagnostics: {
      excludedCodes: strings(diagnostics.excludedCodes, `${path}.diagnostics.excludedCodes`),
      maximum,
      required: requiredDiagnostics,
    },
    excludedRelationIds: strings(item.excludedRelationIds, `${path}.excludedRelationIds`),
    ...(schemaVersion === 1
      ? {}
      : {
          formulaDecision:
            item.formulaDecision === null
              ? null
              : parseFormulaDecisionExpectation(
                  item.formulaDecision,
                  `${path}.formulaDecision`,
                ),
        }),
    proofGrounded: boolean(item.proofGrounded, `${path}.proofGrounded`),
    navigation: {
      definition: parseLocationExpectation(
        navigation.definition,
        `${path}.navigation.definition`,
        schemaVersion,
      ),
      prepareRename: {
        ...(prepareRename.authorization === undefined
          ? {}
          : {
              authorization: oneOf(
                prepareRename.authorization,
                ["authorized", "refused"] as const,
                `${path}.navigation.prepareRename.authorization`,
              ),
            }),
        ...(prepareRename.placeholder === undefined
          ? {}
          : { placeholder: text(prepareRename.placeholder, `${path}.navigation.prepareRename.placeholder`) }),
        ...(prepareRename.range === undefined
          ? {}
          : { range: parseAnchor(prepareRename.range, `${path}.navigation.prepareRename.range`) }),
        status: oneOf(
          prepareRename.status,
          ["available", "unavailable"] as const,
          `${path}.navigation.prepareRename.status`,
        ),
      },
      references: parseLocationExpectation(
        navigation.references,
        `${path}.navigation.references`,
        schemaVersion,
      ),
      rename: {
        ...parseLocationExpectation(rename, `${path}.navigation.rename`, schemaVersion, [
          "expectedText",
          "newName",
          "replacementText",
          "safety",
        ]),
        ...(rename.expectedText === undefined
          ? {}
          : {
              expectedText: text(
                rename.expectedText,
                `${path}.navigation.rename.expectedText`,
              ),
            }),
        ...(rename.newName === undefined
          ? {}
          : { newName: text(rename.newName, `${path}.navigation.rename.newName`) }),
        ...(rename.replacementText === undefined
          ? {}
          : {
              replacementText: text(
                rename.replacementText,
                `${path}.navigation.rename.replacementText`,
              ),
            }),
        ...(rename.safety === undefined
          ? {}
          : { safety: text(rename.safety, `${path}.navigation.rename.safety`) }),
      },
    },
    relations: parseRelationExpectations(item.relations, `${path}.relations`),
    ...(item.symbol === undefined ? {} : { symbol: text(item.symbol, `${path}.symbol`) }),
  };
}

function parseFormulaDecisionExpectation(
  value: unknown,
  path: string,
): NonNullable<AuthoredScientificProbe["expected"]["formulaDecision"]> {
  const item = record(value, path);
  exact(item, ["anchor", "status"], path);
  return {
    anchor: parseAnchor(item.anchor, `${path}.anchor`),
    status: oneOf(
      item.status,
      [
        "ambiguous",
        "conflicting",
        "conventional",
        "engine-limited",
        "established",
        "partial",
        "unsupported",
      ] as const,
      `${path}.status`,
    ),
  };
}

function parseRelationExpectations(
  value: unknown,
  path: string,
): AuthoredRelationExpectation[] {
  const relations = array(value, path).map((value, index) => {
      const relationPath = `${path}[${index}]`;
      const relation = record(value, relationPath);
      exact(relation, ["relationId", "anchor", "roles", "sourceGrounded"], relationPath);
      return {
        anchor: parseAnchor(relation.anchor, `${relationPath}.anchor`),
        relationId: text(relation.relationId, `${relationPath}.relationId`),
        roles: array(relation.roles, `${relationPath}.roles`).map((value, roleIndex) => {
          const rolePath = `${relationPath}.roles[${roleIndex}]`;
          const role = record(value, rolePath);
          exact(role, ["role", "symbol"], rolePath);
          return {
            role: text(role.role, `${rolePath}.role`),
            symbol: text(role.symbol, `${rolePath}.symbol`),
          };
        }),
        sourceGrounded: boolean(relation.sourceGrounded, `${relationPath}.sourceGrounded`),
      };
    });
  return relations;
}

function parseLocationExpectation(
  value: unknown,
  path: string,
  schemaVersion: AuthoredScientificFixture["schemaVersion"] = 1,
  extensions: readonly string[] = [],
): AuthoredLocationExpectation {
  const item = record(value, path);
  const schemaTwoFields = schemaVersion === 2 ? ["allowed", "authorization"] : [];
  exact(
    item,
    ["status", "minimum", "required", "excluded", ...schemaTwoFields, ...extensions],
    path,
    [...schemaTwoFields, ...extensions],
  );
  const result = {
    ...(item.allowed === undefined
      ? {}
      : {
          allowed: array(item.allowed, `${path}.allowed`).map((value, index) =>
            parseAnchor(value, `${path}.allowed[${index}]`)
          ),
        }),
    ...(item.authorization === undefined
      ? {}
      : {
          authorization: oneOf(
            item.authorization,
            ["authorized", "refused"] as const,
            `${path}.authorization`,
          ),
        }),
    excluded: array(item.excluded, `${path}.excluded`).map((value, index) =>
      parseAnchor(value, `${path}.excluded[${index}]`),
    ),
    minimum: integer(item.minimum, `${path}.minimum`),
    required: array(item.required, `${path}.required`).map((value, index) =>
      parseAnchor(value, `${path}.required[${index}]`),
    ),
    status: oneOf(item.status, ["available", "unavailable"] as const, `${path}.status`),
  };
  if (result.minimum < result.required.length) {
    throw new Error(`${path}.minimum: smaller than required anchors`);
  }
  if (result.status === "unavailable" && (result.minimum || result.required.length)) {
    throw new Error(`${path}: unavailable surface cannot require locations`);
  }
  if (result.status === "unavailable" && result.allowed?.length) {
    throw new Error(`${path}: unavailable surface cannot allow locations`);
  }
  return result;
}

function parseAnchor(value: unknown, path: string): AuthoredSourceAnchor {
  const item = record(value, path);
  exact(
    item,
    ["fileId", "needle", "occurrence", "selection"],
    path,
    ["occurrence", "selection"],
  );
  const needle = text(item.needle, `${path}.needle`);
  const selection =
    item.selection === undefined
      ? undefined
      : parseAnchorSelection(item.selection, needle, `${path}.selection`);
  return {
    fileId: text(item.fileId, `${path}.fileId`),
    needle,
    ...(item.occurrence === undefined
      ? {}
      : { occurrence: integer(item.occurrence, `${path}.occurrence`) }),
    ...(selection === undefined ? {} : { selection }),
  };
}

function parseAnchorSelection(
  value: unknown,
  needle: string,
  path: string,
): { readonly length: number; readonly offset: number } {
  const item = record(value, path);
  exact(item, ["offset", "length"], path);
  const offset = integer(item.offset, `${path}.offset`);
  const length = positiveInteger(item.length, `${path}.length`);
  if (offset + length > needle.length) {
    throw new Error(`${path}: selection must fall within the anchor needle`);
  }
  return { length, offset };
}

function validateProbe(
  probe: AuthoredScientificProbe,
  scenario: AuthoredScientificScenario,
  schemaVersion: AuthoredScientificFixture["schemaVersion"],
): void {
  const snapshot = authoredSnapshotFor(scenario, probe);
  const cursor = resolveAuthoredAnchor(snapshot, probe.cursor);
  const cursorOffset =
    probe.cursor.offset === undefined
      ? probe.cursor.edge === "after"
        ? cursor.range.endOffset
        : cursor.range.startOffset
      : cursor.range.startOffset + probe.cursor.offset;
  if (
    probe.cursor.offset !== undefined &&
    (probe.cursor.offset < 0 || probe.cursor.offset > probe.cursor.needle.length)
  ) {
    throw new Error(`${probe.id}: cursor offset falls outside its reviewed needle`);
  }
  for (const location of [
    probe.expected.navigation.definition,
    probe.expected.navigation.references,
    probe.expected.navigation.rename,
  ]) {
    for (const anchor of [
      ...(location.allowed ?? []),
      ...location.required,
      ...location.excluded,
    ]) {
      resolveAuthoredAnchor(snapshot, anchor);
    }
  }
  if (probe.expected.navigation.prepareRename.range) {
    resolveAuthoredAnchor(
      snapshot,
      probe.expected.navigation.prepareRename.range,
    );
  }
  if (probe.expected.formulaDecision) {
    resolveAuthoredAnchor(snapshot, probe.expected.formulaDecision.anchor);
  }
  if (schemaVersion === 2) {
    if (probe.expected.cursorOccurrence === null) {
      if (
        probe.expected.decision !== "unsupported" ||
        probe.expected.proofGrounded ||
        probe.expected.navigation.definition.status !== "unavailable" ||
        probe.expected.navigation.definition.authorization === "authorized" ||
        probe.expected.navigation.references.status !== "unavailable" ||
        probe.expected.navigation.references.authorization === "authorized" ||
        probe.expected.navigation.prepareRename.status !== "unavailable" ||
        probe.expected.navigation.prepareRename.authorization === "authorized" ||
        probe.expected.navigation.rename.status !== "unavailable"
        || probe.expected.navigation.rename.authorization === "authorized"
      ) {
        throw new Error(
          `${probe.id}: a missing cursor occurrence must refuse cursor-entity authority`,
        );
      }
    }
    const rename = probe.expected.navigation.rename;
    if (
      rename.status === "unavailable" &&
      probe.expected.cursorOccurrence !== null &&
      (!rename.newName || !probe.expected.symbol ||
        !authoredRenameNotationFamilyMatches(
          probe.expected.symbol,
          rename.newName,
        ))
    ) {
      throw new Error(
        `${probe.id}: unavailable rename requires a same-family newName`,
      );
    }
  }
  for (const diagnostic of probe.expected.diagnostics.required) {
    resolveAuthoredAnchor(snapshot, diagnostic.anchor);
  }
  for (const relation of probe.expected.relations) {
    const anchor = resolveAuthoredAnchor(snapshot, relation.anchor);
    if (
      anchor.fileId === cursor.fileId &&
      anchor.range.startOffset > cursorOffset
    ) {
      throw new Error(
        `${probe.id}: relation anchor occurs after the cursor evidence boundary`,
      );
    }
  }
}

export function authoredRenameNotationFamilyMatches(
  current: string,
  replacement: string,
): boolean {
  const controlSequence = /^\\\p{L}+$/u;
  const plainIdentifier = /^\p{L}$/u;
  if (controlSequence.test(current)) {
    return controlSequence.test(replacement);
  }
  if (plainIdentifier.test(current)) {
    return plainIdentifier.test(replacement);
  }
  let prefix = 0;
  while (
    prefix < current.length && prefix < replacement.length &&
    current[prefix] === replacement[prefix]
  ) prefix += 1;
  let suffix = 0;
  while (
    suffix < current.length - prefix && suffix < replacement.length - prefix &&
    current[current.length - suffix - 1] ===
      replacement[replacement.length - suffix - 1]
  ) suffix += 1;
  const currentNucleus = current.slice(prefix, current.length - suffix);
  const replacementNucleus = replacement.slice(
    prefix,
    replacement.length - suffix,
  );
  const sharedPrefix = current.slice(0, prefix);
  const sharedSuffix = suffix === 0 ? "" : current.slice(current.length - suffix);
  if (sharedPrefix.endsWith("\\")) {
    return /^\p{L}+$/u.test(currentNucleus) &&
      /^\p{L}+$/u.test(replacementNucleus) &&
      (sharedSuffix === "" || /^[\s_'^]/u.test(sharedSuffix));
  }
  return plainIdentifier.test(currentNucleus) &&
    plainIdentifier.test(replacementNucleus);
}

function requireSplit(fixture: AuthoredScientificFixture, split: AuthoredSplit): void {
  if (fixture.batch.split !== split) throw new Error(`${split} fixture has the wrong split`);
}

function primaryProbes(fixture: AuthoredScientificFixture): AuthoredScientificProbe[] {
  return fixture.probes.filter((probe) => probe.kind === "primary");
}

function validateAreaAllocation(fixture: AuthoredScientificFixture): void {
  for (const field of Object.keys(AUTHORED_AREA_ALLOCATION) as AuthoredArea[]) {
    const expected = AUTHORED_AREA_ALLOCATION[field][fixture.batch.split];
    const actual = fixture.scenarios.filter((scenario) => scenario.field === field).length;
    if (actual !== expected) {
      throw new Error(`${field}: ${fixture.batch.split} requires ${expected} cases, got ${actual}`);
    }
  }
}

function rejectLineageLeakage(
  development: AuthoredScientificFixture,
  holdout: AuthoredScientificFixture,
): void {
  if (development.batch.taskCardDigest === holdout.batch.taskCardDigest) {
    throw new Error("development and holdout task-card batches must differ");
  }
  const developmentGroups = new Set(
    development.scenarios.map((scenario) => scenario.provenance.independenceGroup),
  );
  const projects = new Map<string, string>();
  for (const scenario of [...development.scenarios, ...holdout.scenarios]) {
    if (
      holdout.scenarios.includes(scenario) &&
      developmentGroups.has(scenario.provenance.independenceGroup)
    ) {
      throw new Error(`${scenario.id}: development/holdout author lineage overlap`);
    }
    const normalized = normalizedProject(scenario);
    const existing = projects.get(normalized);
    if (existing) throw new Error(`${scenario.id}: duplicate project with ${existing}`);
    projects.set(normalized, scenario.id);
  }
}

function normalizedProject(scenario: AuthoredScientificScenario): string {
  return scenario.snapshots
    .flatMap((snapshot) =>
      snapshot.documents.map(
        (document) =>
          `${snapshot.id}:${document.path}:${document.content.toLowerCase().replaceAll(/\s+/gu, " ").trim()}`,
      ),
    )
    .join("\n");
}

export function authoredScenarioFor(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificProbe,
): AuthoredScientificScenario {
  const scenario = fixture.scenarios.find((item) => item.id === probe.scenarioId);
  if (!scenario) throw new Error(`${probe.id}: unknown scenario`);
  return scenario;
}

export function authoredSnapshotFor(
  scenario: AuthoredScientificScenario,
  probe: AuthoredScientificProbe,
): AuthoredScientificSnapshot {
  const snapshot = scenario.snapshots.find((item) => item.id === probe.cursor.snapshotId);
  if (!snapshot) throw new Error(`${probe.id}: unknown snapshot ${probe.cursor.snapshotId}`);
  return snapshot;
}

export function resolveAuthoredAnchor(
  snapshot: AuthoredScientificSnapshot,
  anchor: AuthoredSourceAnchor,
): { readonly fileId: string; readonly path: string; readonly range: SourceRange } {
  const document = snapshot.documents.find((item) => item.fileId === anchor.fileId);
  if (!document) throw new Error(`${snapshot.id}: unknown anchor file ${anchor.fileId}`);
  const matches: number[] = [];
  for (let offset = document.content.indexOf(anchor.needle); offset >= 0;) {
    matches.push(offset);
    offset = document.content.indexOf(anchor.needle, offset + Math.max(anchor.needle.length, 1));
  }
  const selected = anchor.occurrence === undefined ? matches[0] : matches[anchor.occurrence];
  if (
    selected === undefined ||
    (anchor.occurrence === undefined && matches.length !== 1)
  ) {
    throw new Error(
      `${snapshot.id}/${anchor.fileId}: anchor needle must identify exactly one occurrence`,
    );
  }
  return {
    fileId: anchor.fileId,
    path: document.path,
    range: {
      startOffset: selected + (anchor.selection?.offset ?? 0),
      endOffset:
        selected +
        (anchor.selection?.offset ?? 0) +
        (anchor.selection?.length ?? anchor.needle.length),
    },
  };
}

export function authoredProbeIdentityMatches(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificProbe,
  observation: AuthoredScientificObservation,
): boolean {
  return authoredProbeIdentityFailures(fixture, probe, observation).length === 0;
}

export function authoredProbeIdentityFailures(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificProbe,
  observation: AuthoredScientificObservation,
): AuthoredIdentityFailure[] {
  const snapshot = authoredSnapshotFor(authoredScenarioFor(fixture, probe), probe);
  const failures: AuthoredIdentityFailure[] = [];
  if (probe.expected.cursorOccurrence !== undefined) {
    const expected = probe.expected.cursorOccurrence;
    if (expected === null) {
      if (observation.symbolLocation) {
        failures.push({
          area: "cursor-symbol",
          basis: "formula-boundary cursor resolved an unexpected symbol occurrence",
        });
      }
    } else {
      const occurrence = resolveAuthoredAnchor(snapshot, expected);
      if (
        !observation.symbolLocation ||
        observation.symbolLocation.fileId !== occurrence.fileId ||
        observation.symbolLocation.path !== occurrence.path ||
        !sameRange(observation.symbolLocation.range, occurrence.range)
      ) {
        failures.push({
          area: "cursor-symbol",
          basis: `cursor occurrence differs from ${expected.fileId}:${expected.needle}`,
        });
      }
    }
  }
  if (probe.expected.symbol && observation.symbol !== probe.expected.symbol) {
    failures.push({
      area: "cursor-symbol",
      basis: `symbol ${observation.symbol ?? "null"}; expected ${probe.expected.symbol}`,
    });
  }
  if (fixture.schemaVersion === 2) {
    checkSurfaceAuthorizations(probe, observation, snapshot, failures);
  }
  if (probe.expected.formulaDecision) {
    const expectedFormula = resolveAuthoredAnchor(
      snapshot,
      probe.expected.formulaDecision.anchor,
    );
    const actualFormula = observation.formulaDecision?.location;
    if (
      !actualFormula || actualFormula.fileId !== expectedFormula.fileId ||
      actualFormula.path !== expectedFormula.path ||
      !sameRange(actualFormula.range, expectedFormula.range)
    ) {
      failures.push({
        area: "formula",
        basis: "selected formula location differs",
      });
    }
  }
  checkLocationExpectation(
    "definition",
    probe.expected.navigation.definition,
    observation.definitions,
    snapshot,
    failures,
  );
  const rename = probe.expected.navigation.rename;
  if (
    rename.expectedText !== undefined &&
    observation.renameEdits.some((edit) => edit.expectedText !== rename.expectedText)
  ) {
    failures.push({ area: "rename", basis: "rename expected source text differs" });
  }
  if (
    rename.replacementText !== undefined &&
    observation.renameEdits.some(
      (edit) => edit.replacementText !== rename.replacementText,
    )
  ) {
    failures.push({ area: "rename", basis: "rename replacement text differs" });
  }
  if (
    rename.safety !== undefined &&
    observation.renameSafety !== rename.safety
  ) {
    failures.push({ area: "rename", basis: "rename safety differs" });
  }
  checkLocationExpectation(
    "references",
    probe.expected.navigation.references,
    observation.references,
    snapshot,
    failures,
  );
  checkLocationExpectation(
    "rename",
    probe.expected.navigation.rename,
    observation.renameEdits,
    snapshot,
    failures,
  );
  const preparation = probe.expected.navigation.prepareRename;
  if (
    Boolean(observation.prepareRename.range) !==
    (preparation.status === "available")
  ) {
    failures.push({
      area: "prepare-rename",
      basis: "prepareRename availability differs",
    });
  }
  if (
    preparation.range &&
    (!observation.prepareRename.range ||
      !sameRange(
        observation.prepareRename.range,
        resolveAuthoredAnchor(snapshot, preparation.range).range,
      ))
  ) {
    failures.push({ area: "prepare-rename", basis: "prepareRename range differs" });
  }
  if (
    preparation.placeholder !== undefined &&
    observation.prepareRename.placeholder !== preparation.placeholder
  ) {
    failures.push({
      area: "prepare-rename",
      basis: "prepareRename placeholder differs",
    });
  }
  return failures;
}

function checkSurfaceAuthorizations(
  probe: AuthoredScientificProbe,
  observation: AuthoredScientificObservation,
  snapshot: AuthoredScientificSnapshot,
  failures: AuthoredIdentityFailure[],
): void {
  const observed = observation.surfaceAuthorizations;
  if (!observed) {
    failures.push({
      area: "cursor-symbol",
      basis: "entity-surface authorizations are missing",
    });
    return;
  }
  const expected = probe.expected.navigation;
  const surfaces = [
    ["definition", observed.definition, expected.definition],
    ["references", observed.references, expected.references],
    ["prepareRename", observed.prepareRename, expected.prepareRename],
    ["rename", observed.rename, expected.rename],
  ] as const;
  for (const [name, authorization, expectation] of surfaces) {
    const expectedStatus = expectation.authorization ??
      (expectation.status === "available" ? "authorized" : "refused");
    if (authorization.status !== expectedStatus) {
      failures.push({
        area: name === "prepareRename" ? "prepare-rename" : name,
        basis: `${name} authorization ${authorization.status}; expected ${expectedStatus}`,
      });
    }
  }
  const authorized = surfaces.flatMap(([, authorization]) =>
    authorization.status === "authorized" ? [authorization] : []
  );
  if (authorized.length === 0) return;
  const expectedOccurrence = probe.expected.cursorOccurrence;
  const expectedLocation = expectedOccurrence === undefined || expectedOccurrence === null
    ? undefined
    : resolveAuthoredAnchor(snapshot, expectedOccurrence);
  const identity = observation.cursorSurfaceIdentity;
  if (
    !expectedLocation || !identity?.entityId ||
    identity.location.fileId !== expectedLocation.fileId ||
    identity.location.path !== expectedLocation.path ||
    !sameRange(identity.location.range, expectedLocation.range) ||
    identity.occurrenceId.fileId !== expectedLocation.fileId ||
    authorized.some((authorization) =>
      stableJson(authorization.focusOccurrenceId) !==
          stableJson(identity.occurrenceId) ||
      stableJson(authorization.entityId) !== stableJson(identity.entityId)
    )
  ) {
    failures.push({
      area: "cursor-symbol",
      basis: "authorized entity surfaces do not match the reviewed cursor identity",
    });
  }
}

function checkLocationExpectation(
  name: "definition" | "references" | "rename",
  expected: AuthoredLocationExpectation,
  actual: readonly ObservedLocation[],
  snapshot: AuthoredScientificSnapshot,
  failures: AuthoredIdentityFailure[],
): boolean {
  let failed = false;
  if ((actual.length > 0) !== (expected.status === "available")) {
    failures.push({ area: name, basis: `${name} availability differs` });
    failed = true;
  }
  if (actual.length < expected.minimum) {
    failures.push({
      area: name,
      basis: `${name} count ${actual.length}; expected at least ${expected.minimum}`,
    });
    failed = true;
  }
  for (const anchor of expected.required) {
    const resolved = resolveAuthoredAnchor(snapshot, anchor);
    if (
      !actual.some(
        (item) =>
          item.fileId === resolved.fileId &&
          item.path === resolved.path &&
          sameRange(item.range, resolved.range),
      )
    ) {
      failures.push({
        area: name,
        basis: `${name} missing ${anchor.fileId}:${anchor.needle}`,
      });
      failed = true;
    }
  }
  for (const anchor of expected.excluded) {
    const resolved = resolveAuthoredAnchor(snapshot, anchor);
    if (
      actual.some(
        (item) =>
          item.fileId === resolved.fileId &&
          item.path === resolved.path &&
          sameRange(item.range, resolved.range),
      )
    ) {
      failures.push({
        area: name,
        basis: `${name} leaked ${anchor.fileId}:${anchor.needle}`,
      });
      failed = true;
    }
  }
  if (expected.allowed) {
    const allowed = expected.allowed.map((anchor) => resolveAuthoredAnchor(snapshot, anchor));
    for (const item of actual) {
      if (
        !allowed.some(
          (candidate) =>
            item.fileId === candidate.fileId &&
            item.path === candidate.path &&
            sameRange(item.range, candidate.range),
        )
      ) {
        failures.push({ area: name, basis: `${name} exposed an unallowed source location` });
        failed = true;
      }
    }
  }
  return failed;
}

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return left.startOffset === right.startOffset && left.endOffset === right.endOffset;
}

/** Accepts an exact semantic range or a reviewed anchor with only surrounding
 * punctuation and TeX presentation metadata. */
export function authoredRelationRangeMatches(
  content: string,
  actual: SourceRange,
  expected: SourceRange,
  formulaEvidence?: SourceRange,
): boolean {
  if (
    formulaEvidence &&
    !sameRange(formulaEvidence, actual) &&
    authoredRelationRangeMatches(content, formulaEvidence, expected)
  ) {
    return true;
  }
  if (sameRange(actual, expected)) return true;
  const metadata = /^(?:(?:\\tag|\\label)\s*\{[^{}]*\}\s*)*$/u;
  const trailing = /^(?:[.,;:]\s*)?(?:(?:\\(?:enspace|medspace|negthinspace|quad|qquad|thickspace|thinspace))\s*)*(?:(?:\\tag|\\label)\s*\{[^{}]*\}\s*)*$/u;
  if (
    actual.startOffset <= expected.startOffset &&
    expected.endOffset <= actual.endOffset
  ) {
    const prefix = content.slice(actual.startOffset, expected.startOffset).trim();
    const suffix = content.slice(expected.endOffset, actual.endOffset).trim();
    return metadata.test(prefix) && trailing.test(suffix);
  }
  if (
    actual.startOffset < expected.startOffset ||
    actual.endOffset > expected.endOffset
  ) {
    return false;
  }
  const prefix = content.slice(expected.startOffset, actual.startOffset).trim();
  const suffix = content.slice(actual.endOffset, expected.endOffset).trim();
  return metadata.test(prefix) && trailing.test(suffix);
}

function countBy<const T extends string>(
  values: readonly T[],
  keys: readonly T[],
): Record<T, number> {
  return Object.fromEntries(
    keys.map((key) => [key, values.filter((value) => value === key).length]),
  ) as Record<T, number>;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path}: must be an array`);
  return value;
}

function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  path: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown) throw new Error(`${path}.${unknown}: unknown field`);
  const missing = keys.find((key) => !optional.includes(key) && !Object.hasOwn(value, key));
  if (missing) throw new Error(`${path}.${missing}: missing field`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${path}: must be non-empty text`);
  return value;
}

function strings(value: unknown, path: string, minimum = 0): string[] {
  const result = array(value, path).map((item, index) => text(item, `${path}[${index}]`));
  if (result.length < minimum) throw new Error(`${path}: requires at least ${minimum} values`);
  unique(result, path);
  return result;
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`${path}: must be a non-negative integer`);
  }
  return value as number;
}

function positiveInteger(value: unknown, path: string): number {
  const result = integer(value, path);
  if (result === 0) throw new Error(`${path}: must be positive`);
  return result;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path}: must be boolean`);
  return value;
}

function digest(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^[0-9a-f]{64}$/u.test(result)) {
    throw new Error(`${path}: must be a lowercase SHA-256 digest`);
  }
  return result;
}

function date(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(result)) throw new Error(`${path}: must be an ISO date`);
  return result;
}

function timestamp(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(result)) {
    throw new Error(`${path}: must be a UTC timestamp`);
  }
  return result;
}

function oneOf<const T extends string>(
  value: unknown,
  values: readonly T[],
  path: string,
): T {
  const result = text(value, path);
  if (!values.includes(result as T)) throw new Error(`${path}: invalid value ${result}`);
  return result as T;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${path}: values must be unique`);
}
