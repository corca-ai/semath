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
