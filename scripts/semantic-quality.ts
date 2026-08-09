export type SemanticQualityField = "formula" | "prose" | "product";

export interface SemanticQualityObservation {
  field: SemanticQualityField;
  domain: string;
  topic: string;
  capability: string;
  cases: number;
  exactCases: number;
  expectedItems: number;
  matchedItems: number;
  actualItems: number;
  unexpectedItems: number;
}

export interface SemanticQualityScore extends SemanticQualityObservation {
  caseAccuracyPercent: number;
  precisionPercent: number;
  recallPercent: number;
}

export interface SemanticQualityBudget {
  id: string;
  selector: Partial<
    Pick<SemanticQualityObservation, "field" | "domain" | "topic" | "capability">
  >;
  minCases?: number;
  minCaseAccuracyPercent?: number;
  minPrecisionPercent?: number;
  minRecallPercent?: number;
  maxUnexpectedItems?: number;
}

export interface SemanticQualityBudgetResult {
  budgetId: string;
  score: SemanticQualityScore;
  violations: string[];
}

type GroupField = "field" | "domain" | "topic" | "capability";

export function aggregateSemanticQuality(
  observations: readonly SemanticQualityObservation[],
  groupBy: readonly GroupField[],
): SemanticQualityScore[] {
  const groups = new Map<string, SemanticQualityObservation>();
  for (const observation of observations) {
    validateObservation(observation);
    const key = groupBy.map((field) => observation[field]).join("\0");
    const aggregate = groups.get(key) ?? {
      field: groupBy.includes("field") ? observation.field : "product",
      domain: groupBy.includes("domain") ? observation.domain : "all",
      topic: groupBy.includes("topic") ? observation.topic : "all",
      capability: groupBy.includes("capability")
        ? observation.capability
        : "all",
      cases: 0,
      exactCases: 0,
      expectedItems: 0,
      matchedItems: 0,
      actualItems: 0,
      unexpectedItems: 0,
    };
    aggregate.cases += observation.cases;
    aggregate.exactCases += observation.exactCases;
    aggregate.expectedItems += observation.expectedItems;
    aggregate.matchedItems += observation.matchedItems;
    aggregate.actualItems += observation.actualItems;
    aggregate.unexpectedItems += observation.unexpectedItems;
    groups.set(key, aggregate);
  }
  return [...groups.values()]
    .map(toScore)
    .sort((left, right) =>
      [left.field, left.domain, left.topic, left.capability]
        .join("\0")
        .localeCompare(
          [right.field, right.domain, right.topic, right.capability].join("\0"),
        ),
    );
}

export function evaluateSemanticQualityBudgets(
  observations: readonly SemanticQualityObservation[],
  budgets: readonly SemanticQualityBudget[],
): SemanticQualityBudgetResult[] {
  const ids = new Set<string>();
  return budgets.map((budget) => {
    if (ids.has(budget.id)) throw new Error(`duplicate quality budget ${budget.id}`);
    ids.add(budget.id);
    const matches = observations.filter((observation) =>
      Object.entries(budget.selector).every(
        ([field, value]) => observation[field as GroupField] === value,
      ),
    );
    const score = aggregateSemanticQuality(matches, [])[0] ?? toScore({
      field: "product",
      domain: "all",
      topic: "all",
      capability: "all",
      cases: 0,
      exactCases: 0,
      expectedItems: 0,
      matchedItems: 0,
      actualItems: 0,
      unexpectedItems: 0,
    });
    const violations: string[] = [];
    minimum(violations, "cases", score.cases, budget.minCases);
    minimum(
      violations,
      "case accuracy",
      score.caseAccuracyPercent,
      budget.minCaseAccuracyPercent,
    );
    minimum(
      violations,
      "precision",
      score.precisionPercent,
      budget.minPrecisionPercent,
    );
    minimum(violations, "recall", score.recallPercent, budget.minRecallPercent);
    if (
      budget.maxUnexpectedItems !== undefined &&
      score.unexpectedItems > budget.maxUnexpectedItems
    ) {
      violations.push(
        `unexpected items ${score.unexpectedItems} exceeds ${budget.maxUnexpectedItems}`,
      );
    }
    return { budgetId: budget.id, score, violations };
  });
}

function toScore(
  observation: SemanticQualityObservation,
): SemanticQualityScore {
  return {
    ...observation,
    caseAccuracyPercent: percent(observation.exactCases, observation.cases),
    precisionPercent:
      observation.actualItems === 0
        ? 100
        : percent(observation.matchedItems, observation.actualItems),
    recallPercent:
      observation.expectedItems === 0
        ? 100
        : percent(observation.matchedItems, observation.expectedItems),
  };
}

function percent(numerator: number, denominator: number) {
  return denominator === 0 ? 100 : Math.round((numerator / denominator) * 1000) / 10;
}

function minimum(
  violations: string[],
  label: string,
  actual: number,
  expected: number | undefined,
) {
  if (expected !== undefined && actual < expected) {
    violations.push(`${label} ${actual} is below ${expected}`);
  }
}

function validateObservation(observation: SemanticQualityObservation) {
  for (const field of [
    "cases",
    "exactCases",
    "expectedItems",
    "matchedItems",
    "actualItems",
    "unexpectedItems",
  ] as const) {
    if (!Number.isInteger(observation[field]) || observation[field] < 0) {
      throw new Error(`${field} must be a non-negative integer`);
    }
  }
  if (observation.exactCases > observation.cases) {
    throw new Error("exactCases cannot exceed cases");
  }
  if (observation.matchedItems > observation.expectedItems) {
    throw new Error("matchedItems cannot exceed expectedItems");
  }
  if (observation.matchedItems > observation.actualItems) {
    throw new Error("matchedItems cannot exceed actualItems");
  }
}
