export const CURSOR_INVARIANT_FAMILIES = [
  "application",
  "binder",
  "declared-surface",
  "derivative",
  "differential",
  "fraction",
  "macro-expansion",
  "malformed",
  "modifier",
  "named-surface",
  "nested-style",
  "plain",
  "style",
  "subscript",
  "superscript",
  "tensor-index",
] as const;

export type CursorInvariantFamily = (typeof CURSOR_INVARIANT_FAMILIES)[number];

export interface CursorInvariantSurface {
  readonly content: string;
  readonly expectedSourceNotation: string;
  readonly expectedSymbol: string;
  readonly family: CursorInvariantFamily;
  readonly fileId: string;
  readonly id: string;
  readonly path: string;
  readonly probes: readonly {
    readonly expectedSourceNotation: string;
    readonly expectedSymbol: string;
    readonly id: string;
    readonly identity: string;
    readonly offset: number;
  }[];
}

interface CursorSurfaceSeed {
  readonly content: string;
  readonly expectedSourceNotation: string;
  readonly expectedSymbol: string;
  readonly family: CursorInvariantFamily;
  readonly id: string;
  readonly probes: readonly {
    readonly delta: number;
    readonly expectedSourceNotation?: string;
    readonly expectedSymbol?: string;
    readonly id: string;
    readonly identity?: string;
    readonly needle: string;
  }[];
}

const SEEDS: readonly CursorSurfaceSeed[] = [
  {
    content:
      "Let $y$ denote the prediction. Compare $\\hat y$ with the observation.",
    expectedSourceNotation: "\\hat y",
    expectedSymbol: "\\hat y",
    family: "modifier",
    id: "unbraced-hat",
    probes: [
      { delta: 0, id: "modifier-start", needle: "\\hat y" },
      { delta: 5, id: "nucleus-start", needle: "\\hat y" },
      { delta: 6, id: "nucleus-after", needle: "\\hat y" },
      { delta: 6, id: "modifier-after", needle: "\\hat y" },
    ],
  },
  {
    content:
      "Let $F$ denote force. Compare $\\mathbf{F}$ with the scalar baseline.",
    expectedSourceNotation: "\\mathbf{F}",
    expectedSymbol: "\\mathbf{F}",
    family: "style",
    id: "styled-force",
    probes: [
      { delta: 8, id: "body-start", needle: "\\mathbf{F}" },
      { delta: 9, id: "body-after", needle: "\\mathbf{F}" },
      { delta: 10, id: "style-after", needle: "\\mathbf{F}" },
    ],
  },
  {
    content:
      "Let $y$ denote the estimate. Compare $\\mathbf{\\hat{y}}$ with the target.",
    expectedSourceNotation: "\\mathbf{\\hat{y}}",
    expectedSymbol: "\\mathbf{\\hat{y}}",
    family: "nested-style",
    id: "nested-style-hat",
    probes: [
      { delta: 13, id: "nucleus-start", needle: "\\mathbf{\\hat{y}}" },
      { delta: 14, id: "nucleus-after", needle: "\\mathbf{\\hat{y}}" },
      { delta: 16, id: "composite-after", needle: "\\mathbf{\\hat{y}}" },
    ],
  },
  {
    content:
      "Expected calibration error (ECE) is reported as $\\operatorname{ECE}(x)$.",
    expectedSourceNotation: "\\operatorname{ECE}",
    expectedSymbol: "ECE",
    family: "named-surface",
    id: "named-ece",
    probes: [
      { delta: 14, id: "name-first", needle: "\\operatorname{ECE}" },
      { delta: 15, id: "name-middle", needle: "\\operatorname{ECE}" },
      { delta: 16, id: "name-last", needle: "\\operatorname{ECE}" },
      { delta: 18, id: "surface-after", needle: "\\operatorname{ECE}" },
    ],
  },
  {
    content:
      "Expected calibration error (ECE) is reported as $\\operatorname{ECE}(x)$.",
    expectedSourceNotation: "\\operatorname{ECE}",
    expectedSymbol: "ECE",
    family: "application",
    id: "ece-application-edge",
    probes: [
      { delta: 0, id: "surface-start", needle: "\\operatorname{ECE}(x)" },
      { delta: 21, id: "application-after", needle: "\\operatorname{ECE}(x)" },
    ],
  },
  {
    content:
      "\\DeclareMathOperator{\\ECE}{ECE}\nExpected calibration error (ECE) is reported as $\\ECE(x)$.",
    expectedSourceNotation: "\\ECE",
    expectedSymbol: "ECE",
    family: "declared-surface",
    id: "declared-ece",
    probes: [
      { delta: 1, id: "call-start", needle: "$\\ECE(x)" },
      { delta: 5, id: "call-after", needle: "$\\ECE(x)" },
      { delta: 8, id: "application-after", needle: "$\\ECE(x)" },
    ],
  },
  {
    content:
      "\\newcommand{\\prediction}[1]{\\hat{#1}}\nLet $y$ denote the prediction. Use $\\prediction{y}$.",
    expectedSourceNotation: "\\prediction{y}",
    expectedSymbol: "\\prediction{y}",
    family: "macro-expansion",
    id: "prediction-macro",
    probes: [
      { delta: 0, id: "call-start", needle: "\\prediction{y}" },
      { delta: 12, id: "argument-start", needle: "\\prediction{y}" },
      { delta: 14, id: "call-after", needle: "\\prediction{y}" },
    ],
  },
  {
    content:
      "Let $A$ and $B$ denote events. Use $p=\\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}$.",
    expectedSourceNotation: "A",
    expectedSymbol: "A",
    family: "fraction",
    id: "fraction-event",
    probes: [
      { delta: 0, id: "symbol-start", needle: "A \\cap B" },
      { delta: 1, id: "symbol-after", needle: "A \\cap B" },
    ],
  },
  {
    content: "Let $x$ denote the input. Compare $x$ with the output.",
    expectedSourceNotation: "x",
    expectedSymbol: "x",
    family: "plain",
    id: "plain-symbol",
    probes: [
      { delta: 0, id: "symbol-start", needle: "x$ with" },
      { delta: 1, id: "symbol-after", needle: "x$ with" },
    ],
  },
  {
    content: "Let $x_i$ denote component i. Compare $x_i$ with the total.",
    expectedSourceNotation: "x_i",
    expectedSymbol: "x_i",
    family: "subscript",
    id: "indexed-component",
    probes: [
      { delta: 0, id: "base-start", needle: "x_i$ with" },
      { delta: 1, id: "base-after", needle: "x_i$ with" },
      {
        delta: 2,
        expectedSourceNotation: "i",
        expectedSymbol: "i",
        id: "index-start",
        identity: "index",
        needle: "x_i$ with",
      },
      {
        delta: 3,
        expectedSourceNotation: "i",
        expectedSymbol: "i",
        id: "index-after",
        identity: "index",
        needle: "x_i$ with",
      },
    ],
  },
  {
    content:
      "Let $x^2$ denote the squared value. Compare $x^2$ with the baseline.",
    expectedSourceNotation: "x^2",
    expectedSymbol: "x^2",
    family: "superscript",
    id: "superscript-value",
    probes: [
      { delta: 0, id: "base-start", needle: "x^2$ with" },
      { delta: 1, id: "base-after", needle: "x^2$ with" },
      { delta: 2, id: "script-start", needle: "x^2$ with" },
      { delta: 3, id: "script-after", needle: "x^2$ with" },
    ],
  },
  {
    content:
      "Let $T_{ij}$ denote the stress tensor component. Compare $T_{ij}$ with zero.",
    expectedSourceNotation: "T_{ij}",
    expectedSymbol: "T_{ij}",
    family: "tensor-index",
    id: "tensor-component",
    probes: [
      { delta: 0, id: "base-start", needle: "T_{ij}$ with" },
      { delta: 1, id: "base-after", needle: "T_{ij}$ with" },
      {
        delta: 3,
        expectedSourceNotation: "i",
        expectedSymbol: "i",
        id: "index-first",
        identity: "index-i",
        needle: "T_{ij}$ with",
      },
      {
        delta: 5,
        expectedSourceNotation: "j",
        expectedSymbol: "j",
        id: "index-after",
        identity: "index-j",
        needle: "T_{ij}$ with",
      },
    ],
  },
  {
    content:
      "Let $y$ denote displacement. Compare $\\dot y$ with the measured rate.",
    expectedSourceNotation: "\\dot y",
    expectedSymbol: "\\dot y",
    family: "derivative",
    id: "dot-derivative",
    probes: [
      { delta: 0, id: "modifier-start", needle: "\\dot y" },
      { delta: 5, id: "nucleus-start", needle: "\\dot y" },
      { delta: 6, id: "surface-after", needle: "\\dot y" },
    ],
  },
  {
    content: "Let $x$ denote position. In $\\frac{dx}{dt}$, inspect $dx$.",
    expectedSourceNotation: "x",
    expectedSymbol: "x",
    family: "differential",
    id: "differential-variable",
    probes: [
      { delta: 1, id: "variable-start", needle: "dx}{dt}" },
      { delta: 2, id: "variable-after", needle: "dx}{dt}" },
    ],
  },
  {
    content: "For $\\sum_{i=1}^{n} x_i$, inspect the bound variable $i$.",
    expectedSourceNotation: "i",
    expectedSymbol: "i",
    family: "binder",
    id: "sum-bound-variable",
    probes: [
      { delta: 0, id: "variable-start", needle: "i=1" },
      { delta: 1, id: "variable-after", needle: "i=1" },
    ],
  },
  {
    content: "Compare the incomplete expression $\\hat{y$ with the baseline.",
    expectedSourceNotation: "\\hat{y",
    expectedSymbol: "\\hat{y",
    family: "malformed",
    id: "incomplete-modifier",
    probes: [
      { delta: 0, id: "nucleus-start", needle: "y$ with" },
      { delta: 1, id: "nucleus-after", needle: "y$ with" },
    ],
  },
] as const;

/** Plans exact UTF-16 probes from reviewed neutral notation surfaces. */
export function planCursorInvariantSurfaces(): readonly CursorInvariantSurface[] {
  const surfaces = SEEDS.map((seed) => ({
    content: seed.content,
    expectedSourceNotation: seed.expectedSourceNotation,
    expectedSymbol: seed.expectedSymbol,
    family: seed.family,
    fileId: `cursor-${seed.id}`,
    id: seed.id,
    path: `cursor-${seed.id}.tex`,
    probes: seed.probes.map((probe) => {
      const start = uniqueNeedleOffset(
        seed.content,
        probe.needle,
        `${seed.id}/${probe.id}`,
      );
      const offset = start + probe.delta;
      if (offset < start || offset > start + probe.needle.length) {
        throw new Error(
          `${seed.id}/${probe.id}: cursor is outside its reviewed surface`,
        );
      }
      return {
        expectedSourceNotation:
          probe.expectedSourceNotation ?? seed.expectedSourceNotation,
        expectedSymbol: probe.expectedSymbol ?? seed.expectedSymbol,
        id: probe.id,
        identity: probe.identity ?? "complete",
        offset,
      };
    }),
  }));
  const families = new Set(surfaces.map((surface) => surface.family));
  for (const family of CURSOR_INVARIANT_FAMILIES) {
    if (!families.has(family))
      throw new Error(`cursor invariant plan is missing ${family}`);
  }
  const ids = surfaces.flatMap((surface) =>
    surface.probes.map((probe) => `${surface.id}/${probe.id}`),
  );
  if (new Set(ids).size !== ids.length)
    throw new Error("cursor invariant probes must be unique");
  return surfaces;
}

function uniqueNeedleOffset(
  content: string,
  needle: string,
  id: string,
): number {
  const first = content.indexOf(needle);
  if (first < 0 || first !== content.lastIndexOf(needle)) {
    throw new Error(`${id}: probe needle must occur exactly once`);
  }
  return first;
}
