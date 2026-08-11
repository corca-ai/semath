import type { ScientificDecision, FirstLossStage } from "./authored-scientific";
import {
  classifyRecognitionFrontier,
  type RecognitionFrontierSignals,
  type RecognitionFrontierStage,
} from "./recognition-frontier";

export interface AuthoredRelationSourceEvidence {
  readonly localRelationMatched: boolean;
  readonly relationId: string;
  readonly signals: RecognitionFrontierSignals;
}

export interface AuthoredFirstLossEvidence {
  readonly cursorSignals: RecognitionFrontierSignals;
  readonly expectedDecision: ScientificDecision;
  readonly expectedRelationsMatched: boolean;
  readonly hostProjectionMismatch?: boolean;
  readonly identityMatches: boolean;
  readonly probePassed: boolean;
  readonly relationSources: readonly AuthoredRelationSourceEvidence[];
}

export interface AuthoredFirstLossObservation {
  readonly basis: string;
  readonly stage: FirstLossStage | null;
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
    return { basis: "all reviewed public surfaces match", stage: null };
  }
  if (evidence.hostProjectionMismatch) {
    return {
      basis: "native semantic surfaces match but a host projection differs",
      stage: "host-projection",
    };
  }
  const observedDecision = evidence.cursorSignals.decision;
  if (
    (observedDecision === "established" || observedDecision === "conflicting") &&
    observedDecision !== evidence.expectedDecision
  ) {
    return {
      basis: `unsafe ${observedDecision} decision differs from reviewed evidence`,
      stage: "decision",
    };
  }
  if (!evidence.expectedRelationsMatched && evidence.relationSources.length) {
    const candidates = evidence.relationSources
      .filter((source) => !source.localRelationMatched)
      .map((source) => ({
        basis: `${source.relationId} is first unavailable at ${authoredStage(
          classifyRecognitionFrontier(source.signals),
        )}`,
        relationId: source.relationId,
        stage: authoredStage(classifyRecognitionFrontier(source.signals)),
      }));
    if (!candidates.length) {
      candidates.push({
        basis:
          "reviewed relations exist at their source equations but not at the observation boundary",
        relationId: "",
        stage: "propagation",
      });
    }
    if (!evidence.identityMatches) {
      candidates.push({
        basis: "symbol identity or a navigation/edit projection differs",
        relationId: "",
        stage: "identity",
      });
    }
    const first = candidates.reduce((left, right) =>
      STAGE_ORDER[left.stage] <= STAGE_ORDER[right.stage] ? left : right,
    );
    return { basis: first.basis, stage: first.stage };
  }
  if (!evidence.identityMatches) {
    return {
      basis: "symbol identity or a navigation/edit projection differs",
      stage: "identity",
    };
  }
  const cursorStage = authoredStage(
    classifyRecognitionFrontier(evidence.cursorSignals),
  );
  if (
    cursorStage !== "decision" &&
    cursorStage !== "pack-unification" &&
    observedDecision !== "established"
  ) {
    return {
      basis: `cursor evidence is first unavailable at ${cursorStage}`,
      stage: cursorStage,
    };
  }
  return {
    basis: "available semantic evidence does not produce the reviewed public decision",
    stage: "decision",
  };
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
