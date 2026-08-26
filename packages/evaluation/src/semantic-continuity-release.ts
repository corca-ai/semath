import type {
  SemanticContinuityDecision,
  SemanticContinuityFixture,
  SemanticContinuityObservation,
  SemanticContinuityScorecard,
} from "./semantic-continuity";

export interface SemanticContinuityDecisionAdjudication {
  readonly caseId: string;
  readonly from: SemanticContinuityDecision;
  readonly to: SemanticContinuityDecision;
}

export interface SemanticContinuityReleaseBaseline {
  readonly cases: number;
  readonly maximumRisk: number;
  readonly minimumPassed: number;
}

export function selectSemanticContinuityFormulaDecisions(
  observations: readonly SemanticContinuityObservation[],
  selectedFormulaCaseIds: readonly string[],
): readonly SemanticContinuityObservation[] {
  const selected = new Set(selectedFormulaCaseIds);
  if (selected.size !== selectedFormulaCaseIds.length) {
    throw new Error(
      "semantic continuity formula selections contain duplicate case IDs",
    );
  }
  const known = new Set(observations.map((item) => item.caseId));
  for (const caseId of selected) {
    if (!known.has(caseId)) {
      throw new Error(`unknown semantic continuity formula selection ${caseId}`);
    }
  }
  return observations.map((item) => {
    if (!selected.has(item.caseId)) return item;
    if (
      item.formulaDecision === null ||
      item.formulaDecision === "conventional" ||
      item.formulaDecision === "engine-limited"
    ) {
      throw new Error(
        `${item.caseId}: selected formula has no legacy continuity decision`,
      );
    }
    return { ...item, decision: item.formulaDecision };
  });
}

export function adjudicateSemanticContinuityDecisions(
  fixture: SemanticContinuityFixture,
  adjudications: readonly SemanticContinuityDecisionAdjudication[],
): SemanticContinuityFixture {
  const byId = new Map(
    adjudications.map((item) => [item.caseId, item] as const),
  );
  if (byId.size !== adjudications.length) {
    throw new Error(
      "semantic continuity adjudications contain duplicate case IDs",
    );
  }
  const known = new Set(fixture.cases.map((item) => item.id));
  for (const item of adjudications) {
    if (!known.has(item.caseId)) {
      throw new Error(
        `unknown semantic continuity adjudication ${item.caseId}`,
      );
    }
  }
  return {
    ...fixture,
    cases: fixture.cases.map((item) => {
      const adjudication = byId.get(item.id);
      if (!adjudication) return item;
      if (item.target.decision !== adjudication.from) {
        throw new Error(
          `${item.id}: adjudication expected ${adjudication.from}, fixture has ${item.target.decision}`,
        );
      }
      return {
        ...item,
        target: { ...item.target, decision: adjudication.to },
      };
    }),
  };
}

export function semanticContinuityReleaseRegressions(
  score: SemanticContinuityScorecard,
  baseline: SemanticContinuityReleaseBaseline,
): readonly string[] {
  const regressions: string[] = [];
  if (score.cases !== baseline.cases) {
    regressions.push(
      `case count ${score.cases} differs from ${baseline.cases}`,
    );
  }
  if (score.passed < baseline.minimumPassed) {
    regressions.push(
      `passed ${score.passed} is below ${baseline.minimumPassed}`,
    );
  }
  if (score.risk.total > baseline.maximumRisk) {
    regressions.push(
      `risk ${score.risk.total} exceeds ${baseline.maximumRisk}`,
    );
  }
  if (score.risk.falseEstablishment > 0) {
    regressions.push(
      `false establishment ${score.risk.falseEstablishment} is unsafe`,
    );
  }
  if (score.risk.falseConflict > 0) {
    regressions.push(`false conflict ${score.risk.falseConflict} is unsafe`);
  }
  if (score.risk.navigationOrIdentity > 0) {
    regressions.push(
      `navigation or identity risk ${score.risk.navigationOrIdentity} is unsafe`,
    );
  }
  return regressions;
}
