import { describe, expect, test } from "bun:test";
import {
  PROPERTY_FAMILIES,
  assertPropertyPlan,
  planPackPropertyCells,
  shrinkPropertyFailure,
} from "./pack-properties";

const packs = [
  {
    laws: [
      {
        id: "balance",
        roles: [{ id: "left" }, { id: "right" }],
        semanticForms: ["left = right", "{left} = {right}"],
      },
    ],
    packId: "test-pack",
  },
] as const;

describe("pack-derived semantic properties", () => {
  test("gives every law every required family deterministically", () => {
    const first = planPackPropertyCells(packs, 20);
    expect(first).toEqual(planPackPropertyCells(packs, 20));
    expect(first.map((cell) => cell.family)).toEqual([...PROPERTY_FAMILIES]);
    expect(new Set(first.map((cell) => cell.id)).size).toBe(first.length);
  });

  test("rejects duplicate cells and impossible source forms", () => {
    const cells = planPackPropertyCells(packs, 20);
    expect(() => assertPropertyPlan([...cells, cells[0]!], packs)).toThrow(
      "duplicate property cell",
    );
    expect(() =>
      planPackPropertyCells(
        [{ packId: "broken", laws: [{ id: "law", roles: [], semanticForms: ["{x"] }] }],
        1,
      ),
    ).toThrow("no renderable semantic form");
  });

  test("shrinks a failure without consulting production matching", () => {
    const cell = {
      ...planPackPropertyCells(packs, 20)[0]!,
      semanticForm: "{{left = right}}",
    };
    expect(
      shrinkPropertyFailure(cell, (candidate) => candidate.semanticForm.includes("="))
        .semanticForm,
    ).toBe("{left = right}");
  });
});
