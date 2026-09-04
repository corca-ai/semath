export interface DifferentialStage<T> {
  readonly name: "clean" | "incremental" | "native" | "wasm" | "worker" | "lsp";
  readonly value: T;
}

export interface DifferentialFailure {
  readonly actual: unknown;
  readonly expected: unknown;
  readonly path: string;
  readonly stage: DifferentialStage<unknown>["name"];
}

export interface EditTraceStep {
  readonly content?: string;
  readonly fileId: string;
  readonly kind: "path-change" | "remove" | "upsert";
  readonly path?: string;
}

export interface EditTrace {
  readonly id: string;
  readonly seed: number;
  readonly steps: readonly EditTraceStep[];
}

export const SEMANTIC_LIFECYCLE_FAMILIES = [
  "declaration-retraction",
  "citation-retraction",
  "conditional-retraction",
  "include-order",
  "macro-retraction",
  "malformed-recovery",
  "negation-retraction",
  "polarity-retraction",
  "typed-conflict-recovery",
  "domain-retraction",
  "formula-attachment-retraction",
] as const;

export type SemanticLifecycleFamily = (typeof SEMANTIC_LIFECYCLE_FAMILIES)[number];

export interface SemanticLifecycleDocument {
  readonly content: string;
  readonly fileId: string;
  readonly path: string;
}

export interface SemanticLifecycleStage {
  readonly changes: readonly EditTraceStep[];
  readonly expectedDecision: "conflicting" | "established" | "not-established";
  readonly id: string;
  readonly queryNeedle?: string;
  readonly expectedDomains?: readonly { readonly packId: string; readonly support: string }[];
}

export interface SemanticLifecycleTrace {
  readonly family: SemanticLifecycleFamily;
  readonly id: string;
  readonly initialDocuments: readonly SemanticLifecycleDocument[];
  readonly initialExpectedDecision: SemanticLifecycleStage["expectedDecision"];
  readonly initialExpectedDomains?: SemanticLifecycleStage["expectedDomains"];
  readonly query: { readonly fileId: string; readonly needle: string };
  readonly seed: number;
  readonly stages: readonly SemanticLifecycleStage[];
}

/** A deterministic edit history that exercises assertion, conflict, retraction and recovery. */
export function planSemanticEditTrace(seed: number): EditTrace {
  if (!Number.isSafeInteger(seed)) throw new Error("trace seed must be an integer");
  const symbol = seed % 2 === 0 ? "x" : "y";
  return {
    id: `semantic-edit-${seed}`,
    seed,
    steps: [
      { content: `Let $${symbol}$ be a vector.`, fileId: "definitions", kind: "upsert", path: "definitions.tex" },
      { content: `\\input{definitions}\n$${symbol}=0$`, fileId: "main", kind: "upsert", path: "main.tex" },
      { content: `Let $${symbol}$ be a vector. Let $${symbol}$ be a matrix.`, fileId: "definitions", kind: "upsert", path: "definitions.tex" },
      { content: `Let $${symbol}$ be a vector.`, fileId: "definitions", kind: "upsert", path: "definitions.tex" },
      { content: `\\input{definitions}\n$${symbol}={` , fileId: "main", kind: "upsert", path: "main.tex" },
      { content: `\\input{definitions}\n$${symbol}=0$`, fileId: "main", kind: "upsert", path: "main.tex" },
      { fileId: "definitions", kind: "path-change", path: "types.tex" },
      { fileId: "definitions", kind: "remove" },
    ],
  };
}

/**
 * Plans independent semantic lifecycles without consulting engine output. The
 * cases establish evidence, remove or contradict it, then recover it so an
 * executor can compare every incremental stage with a clean rebuild.
 */
export function planSemanticLifecycleTraces(seed: number): readonly SemanticLifecycleTrace[] {
  if (!Number.isSafeInteger(seed)) throw new Error("trace seed must be an integer");
  const suffix = Math.abs(seed % 10_000);
  const probabilityDefinitions =
    "Let $A$ and $B$ be events in the same probability space.";
  const probabilityMain = "\\input{definitions}\n$A \\cap B$.";
  const localProbability = `${probabilityDefinitions}\n$A \\cap B$.`;
  const traces: SemanticLifecycleTrace[] = [
    {
      family: "domain-retraction",
      id: `lifecycle-${suffix}-domain`,
      initialDocuments: [{ content: "A probability distribution is considered.\n$q$", fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "not-established",
      initialExpectedDomains: [{ packId: "probability", support: "tentative" }],
      query: { fileId: "main", needle: "q" },
      seed,
      stages: [
        {
          changes: [{ content: "Only editorial notation remains.\n$q$", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          expectedDomains: [],
          id: "remove-domain-evidence",
        },
        {
          changes: [{ content: "A probability distribution is considered.\n$q$", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          expectedDomains: [{ packId: "probability", support: "tentative" }],
          id: "restore-domain-evidence",
        },
      ],
    },
    {
      family: "formula-attachment-retraction",
      id: `lifecycle-${suffix}-attachment`,
      initialDocuments: [{ content: "$V=IR$, where $V$ denotes voltage, $I$ electric current, and $R$ resistance.", fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "not-established",
      initialExpectedDomains: [{ packId: "circuits", support: "explicit" }],
      query: { fileId: "main", needle: "V=IR" },
      seed,
      stages: [
        {
          changes: [{ content: "$V=IR$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          expectedDomains: [],
          id: "remove-attached-prose",
        },
        {
          changes: [{ content: "$V=IR$, where $V$ denotes voltage, $I$ electric current, and $R$ resistance.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          expectedDomains: [{ packId: "circuits", support: "explicit" }],
          id: "restore-attached-prose",
        },
      ],
    },
    {
      family: "declaration-retraction",
      id: `lifecycle-${suffix}-declaration`,
      initialDocuments: [
        { content: probabilityMain, fileId: "main", path: "main.tex" },
        { content: probabilityDefinitions, fileId: "definitions", path: "definitions.tex" },
      ],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ fileId: "definitions", kind: "remove" }],
          expectedDecision: "not-established",
          id: "remove-evidence",
        },
        {
          changes: [{ content: probabilityDefinitions, fileId: "definitions", kind: "upsert", path: "definitions.tex" }],
          expectedDecision: "established",
          id: "restore-evidence",
        },
      ],
    },
    {
      family: "citation-retraction",
      id: `lifecycle-${suffix}-citation`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "Prior work \\parencite{study} reports that $A$ and $B$ are events.\n$A \\cap B$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "attribute-declaration",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "restore-author-assertion",
        },
      ],
    },
    {
      family: "conditional-retraction",
      id: `lifecycle-${suffix}-conditional`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "If $A$ and $B$ were events, the operation would be defined.\n$A \\cap B$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "condition-declaration",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "restore-unconditional-declaration",
        },
      ],
    },
    {
      family: "include-order",
      id: `lifecycle-${suffix}-include-order`,
      initialDocuments: [
        { content: probabilityMain, fileId: "main", path: "main.tex" },
        { content: probabilityDefinitions, fileId: "definitions", path: "definitions.tex" },
      ],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "$A \\cap B$.\n\\input{definitions}", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "move-evidence-after-use",
        },
        {
          changes: [{ content: probabilityMain, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "restore-include-order",
        },
      ],
    },
    {
      family: "macro-retraction",
      id: `lifecycle-${suffix}-macro`,
      initialDocuments: [
        { content: "\\newcommand{\\joint}[2]{#1 \\cap #2}", fileId: "macros", path: "macros.tex" },
        { content: `\\input{macros}\n${probabilityDefinitions}\n$\\joint{A}{B}$.`, fileId: "main", path: "main.tex" },
      ],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "\\joint{A}{B}" },
      seed,
      stages: [
        {
          changes: [{ fileId: "macros", kind: "remove" }],
          expectedDecision: "not-established",
          id: "remove-macro-definition",
        },
        {
          changes: [{ content: "\\newcommand{\\joint}[2]{#1 \\cap #2}", fileId: "macros", kind: "upsert", path: "macros.tex" }],
          expectedDecision: "established",
          id: "restore-macro-definition",
        },
      ],
    },
    {
      family: "malformed-recovery",
      id: `lifecycle-${suffix}-malformed`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: `${probabilityDefinitions}\n$A \\cap$.`, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "break-expression",
          queryNeedle: "\\cap",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "repair-expression",
        },
      ],
    },
    {
      family: "polarity-retraction",
      id: `lifecycle-${suffix}-polarity`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "A and B might be events in the same probability space.\n$A \\cap B$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "hedge-declaration",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "restore-assertion",
        },
      ],
    },
    {
      family: "negation-retraction",
      id: `lifecycle-${suffix}-negation`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "$A$ and $B$ are not events in the same probability space.\n$A \\cap B$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "not-established",
          id: "negate-declaration",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "restore-positive-declaration",
        },
      ],
    },
    {
      family: "typed-conflict-recovery",
      id: `lifecycle-${suffix}-conflict`,
      initialDocuments: [{ content: localProbability, fileId: "main", path: "main.tex" }],
      initialExpectedDecision: "established",
      query: { fileId: "main", needle: "A \\cap B" },
      seed,
      stages: [
        {
          changes: [{ content: "Let $e$ be kinetic energy, $Z$ mass, and $k$ speed.\n$e=Zk$.", fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "conflicting",
          id: "introduce-explicit-conflict",
          queryNeedle: "e=Zk",
        },
        {
          changes: [{ content: localProbability, fileId: "main", kind: "upsert", path: "main.tex" }],
          expectedDecision: "established",
          id: "remove-conflict",
        },
      ],
    },
  ];
  if (new Set(traces.map((trace) => trace.family)).size !== SEMANTIC_LIFECYCLE_FAMILIES.length) {
    throw new Error("lifecycle trace plan is missing a required family");
  }
  return traces;
}

export function firstDifferentialFailure<T>(
  stages: readonly DifferentialStage<T>[],
): DifferentialFailure | undefined {
  const reference = stages[0];
  if (!reference) return undefined;
  for (const stage of stages.slice(1)) {
    const difference = firstDifference(reference.value, stage.value, "$");
    if (difference) return { ...difference, stage: stage.name };
  }
  return undefined;
}

export function shrinkEditTrace(
  trace: EditTrace,
  stillFails: (candidate: EditTrace) => boolean,
): EditTrace {
  let steps = [...trace.steps];
  for (let index = steps.length - 1; index >= 0; index -= 1) {
    const candidate = { ...trace, steps: steps.filter((_, item) => item !== index) };
    if (candidate.steps.length && stillFails(candidate)) steps = [...candidate.steps];
  }
  return { ...trace, steps };
}

function firstDifference(
  expected: unknown,
  actual: unknown,
  path: string,
): { actual: unknown; expected: unknown; path: string } | undefined {
  if (Object.is(expected, actual)) return undefined;
  if (Array.isArray(expected) && Array.isArray(actual)) {
    if (expected.length !== actual.length) return { actual, expected, path: `${path}.length` };
    for (const [index, value] of expected.entries()) {
      const difference = firstDifference(value, actual[index], `${path}[${index}]`);
      if (difference) return difference;
    }
    return undefined;
  }
  if (isRecord(expected) && isRecord(actual)) {
    const keys = [...new Set([...Object.keys(expected), ...Object.keys(actual)])].sort();
    for (const key of keys) {
      const difference = firstDifference(expected[key], actual[key], `${path}.${key}`);
      if (difference) return difference;
    }
    return undefined;
  }
  return { actual, expected, path };
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
