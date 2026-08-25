import { describe, expect, test } from "bun:test";
import { observedQualityRelations } from "./runner";

const selected = {
  documentVersion: 1,
  location: {
    fileId: "main",
    path: "main.tex",
    range: { endOffset: 8, startOffset: 3 },
  },
  scopePath: [1],
  sourceNotation: "x=1",
};

function hypothesis(
  relationId: string,
  support: "contradicted" | "derived" | "explicit" | "supported" | "tentative",
  formula = selected,
  kind: "reviewed-convention" | "typed-law" = "typed-law",
  symbol = "x",
) {
  return {
    conditions: [],
    formula,
    kind,
    relation: {
      conditions: [],
      description: relationId,
      evidence: [{
        kind: "source-claim",
        ruleId: "test/source",
        sourceRanges: [{ endOffset: 8, startOffset: 3 }],
        strength: "asserted",
      }],
      range: { endOffset: 8, startOffset: 3 },
      relationId,
      roles: [{ label: "value", role: "value", symbol }],
      title: relationId,
    },
    support,
  } as const;
}

describe("quality formula observation", () => {
  test("counts exact selected-formula recognition without granting authority", () => {
    expect(
      observedQualityRelations({
        formula: selected,
        interpretations: {
          hypotheses: [
            hypothesis("test:supported", "supported"),
            hypothesis("test:tentative", "tentative"),
            hypothesis("test:contradicted", "contradicted"),
          ],
        },
      }).map((relation) => relation.relationId),
    ).toEqual(["test:supported", "test:tentative"]);
  });

  test("rejects a supported sibling-formula hypothesis", () => {
    expect(
      observedQualityRelations({
        formula: selected,
        interpretations: {
          hypotheses: [
            hypothesis("test:sibling", "supported", {
              ...selected,
              location: {
                ...selected.location,
                range: { endOffset: 14, startOffset: 10 },
              },
            }),
          ],
        },
      }),
    ).toEqual([]);
  });

  test("rejects reviewed conventions and stale formula notation", () => {
    expect(
      observedQualityRelations({
        formula: selected,
        interpretations: {
          hypotheses: [
            hypothesis("test:convention", "supported", selected, "reviewed-convention"),
            hypothesis("test:stale", "explicit", {
              ...selected,
              sourceNotation: "y=1",
            }),
          ],
        },
      }),
    ).toEqual([]);
  });

  test("preserves distinct same-law role bindings", () => {
    expect(
      observedQualityRelations({
        formula: selected,
        interpretations: {
          hypotheses: [
            hypothesis("test:relation", "explicit", selected, "typed-law", "x"),
            hypothesis("test:relation", "explicit", selected, "typed-law", "y"),
          ],
        },
      }).map((relation) => relation.roles[0]?.symbol),
    ).toEqual(["x", "y"]);
  });
});
