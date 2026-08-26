import type {
  AuthoredIdentityFailure,
  DocumentReasoningFamily,
  ScientificDecision,
  FirstLossStage,
} from "./authored-scientific";
import type { MathAuthoringDisposition } from "../../protocol/src/index";
import {
  classifyRecognitionFrontier,
  type RecognitionFrontierSignals,
  type RecognitionFrontierStage,
} from "./recognition-frontier";

export interface AuthoredRelationSourceEvidence {
  readonly localRelationMatched: boolean;
  readonly rangeMatched: boolean;
  readonly relationId: string;
  readonly relationPresent: boolean;
  readonly rolesMatched: boolean;
  readonly signals: RecognitionFrontierSignals;
}

export interface AuthoredFirstLossEvidence {
  readonly cursorSignals: RecognitionFrontierSignals;
  readonly expectedDecision: ScientificDecision;
  readonly expectedFormulaDecision?: MathAuthoringDisposition;
  readonly expectedRelationsMatched: boolean;
  readonly hostProjectionMismatch?: boolean;
  readonly formulaDecision?: MathAuthoringDisposition;
  readonly formulaLocationMatched?: boolean;
  readonly identityFailures: readonly AuthoredIdentityFailure[];
  readonly probePassed: boolean;
  readonly relationSources: readonly AuthoredRelationSourceEvidence[];
}

export interface AuthoredFirstLossObservation {
  readonly basis: string;
  readonly decisionDomain?: "cursor-entity" | "selected-formula";
  readonly reason: AuthoredFirstLossReason;
  readonly stage: FirstLossStage | null;
}

export const AUTHORED_FIRST_LOSS_REASONS = [
  "passed",
  "host-projection-mismatch",
  "unsafe-decision",
  "neutral-syntax-unavailable",
  "discourse-evidence-missing",
  "cursor-occurrence-mismatch",
  "formula-selection-mismatch",
  "entity-scope-unresolved",
  "navigation-projection-mismatch",
  "edit-projection-mismatch",
  "canonical-ir-engine-limit",
  "typed-fact-condition-missing",
  "propagation-boundary-loss",
  "structural-dispatch-miss",
  "relation-range-mismatch",
  "unresolved-collision",
  "role-or-equivalent-form-miss",
  "evidence-sufficiency-mismatch",
] as const;

export type AuthoredFirstLossReason =
  (typeof AUTHORED_FIRST_LOSS_REASONS)[number];

export interface AuthoredFirstLossRecord extends AuthoredFirstLossObservation {
  readonly caseId: string;
  readonly expectedDecision: ScientificDecision;
  readonly expectedFormulaDecision?: MathAuthoringDisposition;
  readonly family: DocumentReasoningFamily;
  readonly field: string;
  readonly split: "development" | "holdout";
}

export interface AuthoredFirstLossCount {
  readonly key: string;
  readonly count: number;
}

export interface AuthoredFirstLossAtlas {
  readonly failed: number;
  readonly passed: number;
  readonly byDecision: readonly AuthoredFirstLossCount[];
  readonly byEntityDecision: readonly AuthoredFirstLossCount[];
  readonly byFormulaDecision: readonly AuthoredFirstLossCount[];
  readonly byFamily: readonly AuthoredFirstLossCount[];
  readonly byField: readonly AuthoredFirstLossCount[];
  readonly byReason: readonly AuthoredFirstLossCount[];
  readonly bySplit: readonly AuthoredFirstLossCount[];
  readonly byStage: readonly AuthoredFirstLossCount[];
  readonly total: number;
}

const STAGE_ORDER: Readonly<Record<FirstLossStage, number>> = {
  "neutral-syntax": 0,
  attachment: 1,
  identity: 2,
  "canonical-ir": 3,
  "typed-fact": 4,
  propagation: 5,
  "pack-unification": 6,
  decision: 7,
  "host-projection": 8,
};

/**
 * Localizes a failed authored probe from public semantic evidence. This is an
 * evaluation projection over the existing recognition frontier, not another
 * runtime recognizer or inference path.
 */
export function classifyAuthoredFirstLoss(
  evidence: AuthoredFirstLossEvidence,
): AuthoredFirstLossObservation {
  if (evidence.probePassed) {
    return {
      basis: "all reviewed public surfaces match",
      reason: "passed",
      stage: null,
    };
  }
  const observedDecision = evidence.cursorSignals.decision;
  if (
    (observedDecision === "established" || observedDecision === "conflicting") &&
    observedDecision !== evidence.expectedDecision
  ) {
    return {
      basis: `unsafe ${observedDecision} decision differs from reviewed evidence`,
      decisionDomain: "cursor-entity",
      reason: "unsafe-decision",
      stage: "decision",
    };
  }
  const candidates: LossCandidate[] = [];
  if (evidence.hostProjectionMismatch) {
    candidates.push({
      basis: "native semantic surfaces match but a host projection differs",
      decisionDomain: "cursor-entity",
      reason: "host-projection-mismatch",
      stage: "host-projection",
    });
  }
  if (evidence.formulaLocationMatched === false) {
    candidates.push({
      basis: "selected formula location differs from reviewed evidence",
      decisionDomain: "selected-formula",
      reason: "formula-selection-mismatch",
      stage: "identity",
    });
  }
  if (
    evidence.expectedFormulaDecision !== undefined &&
    evidence.formulaDecision !== undefined &&
    evidence.formulaDecision !== evidence.expectedFormulaDecision
  ) {
    candidates.push({
      basis:
        (evidence.formulaDecision === "established" ||
            evidence.formulaDecision === "conflicting")
          ? `unsafe ${evidence.formulaDecision} selected-formula decision differs from reviewed evidence`
          : "selected formula evidence does not produce the reviewed decision",
      decisionDomain: "selected-formula",
      reason:
        evidence.formulaDecision === "established" ||
          evidence.formulaDecision === "conflicting"
          ? "unsafe-decision"
          : "evidence-sufficiency-mismatch",
      stage: "decision",
    });
  }
  if (!evidence.expectedRelationsMatched && evidence.relationSources.length) {
    const relationCandidates = evidence.relationSources
      .filter((source) => !source.localRelationMatched)
      .map(relationSourceLoss);
    if (!relationCandidates.length) {
      relationCandidates.push({
        basis:
          "reviewed relations exist at their source equations but not at the observation boundary",
        decisionDomain: "selected-formula",
        relationId: "",
        reason: "propagation-boundary-loss",
        stage: "propagation",
      });
    }
    candidates.push(...relationCandidates);
  }
  if (evidence.identityFailures.length) {
    const identity = identityLoss(evidence.identityFailures);
    candidates.push({ ...identity, stage: stageForIdentityLoss(identity.reason) });
  }
  if (observedDecision !== evidence.expectedDecision) {
    const cursorStage = authoredStage(
      classifyRecognitionFrontier(evidence.cursorSignals),
    );
    candidates.push({
      basis: `cursor evidence is first unavailable at ${cursorStage}`,
      decisionDomain: "cursor-entity",
      reason: reasonForFrontier(evidence.cursorSignals),
      stage: cursorStage,
    });
  }
  if (candidates.length) {
    const first = candidates.reduce((left, right) =>
      STAGE_ORDER[left.stage] <= STAGE_ORDER[right.stage] ? left : right,
    );
    return first;
  }
  return {
    basis: "available semantic evidence does not produce the reviewed public decision",
    decisionDomain: "cursor-entity",
    reason: "evidence-sufficiency-mismatch",
    stage: "decision",
  };
}

type LossCandidate = AuthoredFirstLossObservation & {
  readonly decisionDomain: "cursor-entity" | "selected-formula";
  readonly stage: FirstLossStage;
};

function relationSourceLoss(source: AuthoredRelationSourceEvidence): {
  readonly basis: string;
  readonly decisionDomain: "selected-formula";
  readonly reason: AuthoredFirstLossReason;
  readonly relationId: string;
  readonly stage: FirstLossStage;
} {
  if (source.relationPresent && !source.rangeMatched) {
    return {
      basis: `${source.relationId} is recognized outside its reviewed source range`,
      decisionDomain: "selected-formula",
      reason: "relation-range-mismatch",
      relationId: source.relationId,
      stage: "pack-unification",
    };
  }
  if (source.rangeMatched && !source.rolesMatched) {
    return {
      basis: `${source.relationId} has a reviewed source range but different role bindings`,
      decisionDomain: "selected-formula",
      reason: "role-or-equivalent-form-miss",
      relationId: source.relationId,
      stage: "pack-unification",
    };
  }
  const reason = reasonForRelationFrontier(source.signals);
  return {
    basis: `${source.relationId} is first unavailable at ${stageForReason(reason)}`,
    decisionDomain: "selected-formula",
    reason,
    relationId: source.relationId,
    stage: stageForReason(reason),
  };
}

function reasonForRelationFrontier(
  signals: RecognitionFrontierSignals,
): AuthoredFirstLossReason {
  if (!signals.syntaxAvailable) return "neutral-syntax-unavailable";
  if (signals.engineLimited) return "canonical-ir-engine-limit";
  if (!signals.discourseEvidence) return "discourse-evidence-missing";
  if (!signals.structuralCandidates) return "structural-dispatch-miss";
  if (!signals.typeOrConditionEvidence) return "typed-fact-condition-missing";
  if (signals.decision === "ambiguous") return "unresolved-collision";
  return "role-or-equivalent-form-miss";
}

function stageForReason(reason: AuthoredFirstLossReason): FirstLossStage {
  switch (reason) {
    case "neutral-syntax-unavailable":
      return "neutral-syntax";
    case "canonical-ir-engine-limit":
      return "canonical-ir";
    case "discourse-evidence-missing":
      return "attachment";
    case "typed-fact-condition-missing":
      return "typed-fact";
    case "structural-dispatch-miss":
    case "relation-range-mismatch":
    case "unresolved-collision":
    case "role-or-equivalent-form-miss":
      return "pack-unification";
    default:
      return "decision";
  }
}

export function summarizeAuthoredFirstLoss(
  records: readonly AuthoredFirstLossRecord[],
): AuthoredFirstLossAtlas {
  const failed = records.filter((record) => record.stage !== null);
  const missingDomain = failed.find(
    (record) => record.decisionDomain === undefined,
  );
  if (missingDomain) {
    throw new Error(
      `${missingDomain.caseId}: failed first-loss record lacks a decision domain`,
    );
  }
  const entityFailures = failed.filter(
    (record) => record.decisionDomain === "cursor-entity",
  );
  const formulaFailures = failed.filter(
    (record) => record.decisionDomain === "selected-formula",
  );
  return {
    failed: failed.length,
    passed: records.length - failed.length,
    byDecision: counts(
      failed.map((record) =>
        record.decisionDomain === "selected-formula"
          ? record.expectedFormulaDecision ?? record.expectedDecision
          : record.expectedDecision
      ),
    ),
    byEntityDecision: counts(
      entityFailures.map((record) => record.expectedDecision),
    ),
    byFormulaDecision: counts(
      formulaFailures.flatMap((record) =>
        record.expectedFormulaDecision === undefined
          ? []
          : [record.expectedFormulaDecision]
      ),
    ),
    byFamily: counts(failed.map((record) => record.family)),
    byField: counts(failed.map((record) => record.field)),
    byReason: counts(failed.map((record) => record.reason)),
    bySplit: counts(failed.map((record) => record.split)),
    byStage: counts(failed.flatMap((record) => (record.stage ? [record.stage] : []))),
    total: records.length,
  };
}

function counts(values: readonly string[]): AuthoredFirstLossCount[] {
  const result = new Map<string, number>();
  for (const value of values) result.set(value, (result.get(value) ?? 0) + 1);
  return [...result]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, count]) => ({ key, count }));
}

function identityLoss(
  failures: readonly AuthoredIdentityFailure[],
): {
  readonly basis: string;
  readonly decisionDomain: "cursor-entity" | "selected-formula";
  readonly reason: AuthoredFirstLossReason;
} {
  const areas = new Set(failures.map((failure) => failure.area));
  const basis = failures.map((failure) => failure.basis).join("; ");
  if (areas.has("cursor-symbol")) {
    return {
      basis,
      decisionDomain: "cursor-entity",
      reason: "cursor-occurrence-mismatch",
    };
  }
  if (areas.has("formula")) {
    return {
      basis,
      decisionDomain: "selected-formula",
      reason: "formula-selection-mismatch",
    };
  }
  if (areas.has("definition") || areas.has("references")) {
    return {
      basis,
      decisionDomain: "cursor-entity",
      reason: "navigation-projection-mismatch",
    };
  }
  return {
    basis,
    decisionDomain: "cursor-entity",
    reason: "edit-projection-mismatch",
  };
}

function stageForIdentityLoss(reason: AuthoredFirstLossReason): FirstLossStage {
  return reason === "navigation-projection-mismatch" ||
      reason === "edit-projection-mismatch"
    ? "host-projection"
    : "identity";
}

function reasonForFrontier(
  signals: RecognitionFrontierSignals,
): AuthoredFirstLossReason {
  const stage = classifyRecognitionFrontier(signals);
  switch (stage) {
    case "syntax-unavailable":
      return "neutral-syntax-unavailable";
    case "discourse-evidence-missing":
      return "discourse-evidence-missing";
    case "identity-scope-unresolved":
      return "entity-scope-unresolved";
    case "canonical-unsupported":
      return "canonical-ir-engine-limit";
    case "type-condition-evidence-missing":
      return "typed-fact-condition-missing";
    case "structural-candidate-missing":
      return "structural-dispatch-miss";
    case "genuine-ambiguity":
      return "unresolved-collision";
    case "demonstrated-conflict":
    case "established":
      return "evidence-sufficiency-mismatch";
  }
}

function authoredStage(stage: RecognitionFrontierStage): FirstLossStage {
  switch (stage) {
    case "syntax-unavailable":
      return "neutral-syntax";
    case "canonical-unsupported":
      return "canonical-ir";
    case "discourse-evidence-missing":
      return "attachment";
    case "identity-scope-unresolved":
      return "identity";
    case "structural-candidate-missing":
    case "genuine-ambiguity":
      return "pack-unification";
    case "type-condition-evidence-missing":
      return "typed-fact";
    case "demonstrated-conflict":
    case "established":
      return "decision";
  }
}
