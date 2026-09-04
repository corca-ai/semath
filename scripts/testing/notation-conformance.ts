import type {
  LatexNotationNode,
  LatexNotationNodeKind,
  LatexSyntaxState,
} from "wasmtex/syntax";

export const NOTATION_COVERAGE_FACETS = [
  "structuralFamily",
  "surface",
  "context",
  "cursorTarget",
  "outcome",
] as const;
export type NotationCoverageFacet = (typeof NOTATION_COVERAGE_FACETS)[number];
const NOTATION_NODE_KINDS = [
  "token",
  "sequence",
  "group",
  "command",
  "script",
  "delimiter",
  "alignment",
  "environment",
  "modifier",
  "style",
  "named-operator",
  "opaque",
  "error",
] as const satisfies readonly LatexNotationNodeKind[];
const NOTATION_STATES = [
  "complete",
  "incomplete",
  "ambiguous",
  "opaque",
  "cyclic",
  "truncated",
] as const satisfies readonly LatexSyntaxState[];
type ProvenanceOrigin = NonNullable<LatexNotationNode["provenance"]>["origin"];
const PROVENANCE_ORIGINS = [
  "source",
  "call-site",
  "definition",
  "expansion",
  "generated",
] as const satisfies readonly ProvenanceOrigin[];

export interface NotationNodeExpectation {
  kind: LatexNotationNodeKind;
  name?: string;
  provenanceOrigin?: ProvenanceOrigin;
  state?: LatexSyntaxState;
  text?: string;
}

export interface NotationConformanceCase {
  id: string;
  language: "latex" | "markdown";
  content: string;
  cursor: { needle: string; offset: number };
  expectedAncestor: NotationNodeExpectation;
  forbiddenAncestor?: NotationNodeExpectation;
  coverage: Readonly<Record<NotationCoverageFacet, string>>;
}

export interface NotationConformanceCorpus {
  schemaVersion: 1;
  requiredCoverage: Readonly<Record<NotationCoverageFacet, readonly string[]>>;
  cases: readonly NotationConformanceCase[];
}

export function parseNotationConformanceCorpus(value: unknown): NotationConformanceCorpus {
  const root = record(value, "notation corpus");
  exactKeys(root, ["schemaVersion", "requiredCoverage", "cases"], "notation corpus");
  if (root.schemaVersion !== 1) fail("notation corpus.schemaVersion", "must be 1");
  const requiredCoverage = parseRequiredCoverage(root.requiredCoverage);
  const cases = array(root.cases, "notation corpus.cases").map((item, index) =>
    parseCase(item, `notation corpus.cases[${index}]`),
  );
  if (cases.length === 0) fail("notation corpus.cases", "must not be empty");
  unique(cases.map((item) => item.id), "notation corpus.cases");
  return { schemaVersion: 1, requiredCoverage, cases };
}

export function notationCoverageGaps(corpus: NotationConformanceCorpus): string[] {
  const gaps: string[] = [];
  for (const facet of NOTATION_COVERAGE_FACETS) {
    const observed = new Set(corpus.cases.map((item) => item.coverage[facet]));
    for (const required of corpus.requiredCoverage[facet]) {
      if (!observed.has(required)) gaps.push(`${facet}:${required}`);
    }
  }
  return gaps;
}

export function matchesNotationExpectation(
  node: LatexNotationNode,
  expectation: NotationNodeExpectation,
): boolean {
  return (
    node.kind === expectation.kind &&
    (expectation.name === undefined || node.name === expectation.name) &&
    (expectation.text === undefined || node.text === expectation.text) &&
    (expectation.state === undefined || node.state === expectation.state) &&
    (expectation.provenanceOrigin === undefined ||
      (node.provenance?.origin ?? "source") === expectation.provenanceOrigin)
  );
}

export function notationArenaFailures(
  nodes: readonly LatexNotationNode[],
  sourceLength: number,
): string[] {
  const failures: string[] = [];
  for (const [index, node] of nodes.entries()) {
    const { startOffset, endOffset } = node.ranges.full;
    if (startOffset < 0 || endOffset < startOffset || endOffset > sourceLength) {
      failures.push(`node ${index}: invalid full range ${startOffset}..${endOffset}`);
    }
    for (const child of node.children) {
      const childNode = nodes[child];
      if (!childNode) {
        failures.push(`node ${index}: missing child ${child}`);
      } else if (childNode.parent !== index) {
        failures.push(`node ${index}: child ${child} points to parent ${childNode.parent}`);
      }
    }
    for (let childIndex = 1; childIndex < node.children.length; childIndex++) {
      const previous = nodes[node.children[childIndex - 1]!];
      const current = nodes[node.children[childIndex]!];
      if (
        previous &&
        current &&
        previous.ranges.full.startOffset > current.ranges.full.startOffset
      ) {
        failures.push(`node ${index}: children are not source ordered`);
      }
    }
  }
  return failures;
}

export function generateNotationFuzzSources(seed: number, count: number): readonly string[] {
  if (!Number.isSafeInteger(seed) || !Number.isSafeInteger(count) || count < 1 || count > 1_000) {
    throw new Error("notation fuzz seed/count must be bounded positive integers");
  }
  let state = seed >>> 0;
  const next = (): number => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state;
  };
  const atoms = [
    "x",
    "𝑦",
    "\\hat{x}",
    "\\operatorname{ECE}",
    "B_m",
    "\\frac{x}{y}",
    "[a,b]",
    "\\unknown{x}",
  ];
  const sources: string[] = [];
  for (let index = 0; index < count; index++) {
    const depth = 1 + (next() % 24);
    let expression = atoms[next() % atoms.length]!;
    for (let level = 0; level < depth; level++) {
      const choice = next() % 6;
      if (choice === 0) expression = `{${expression}}`;
      else if (choice === 1) expression = `\\hat{${expression}}`;
      else if (choice === 2) expression = `[${expression}]`;
      else if (choice === 3) expression = `${expression}_i`;
      else if (choice === 4) expression = `\\mathbf{${expression}}`;
      else expression = `${expression}+${atoms[next() % atoms.length]}`;
    }
    const malformed = next() % 4 === 0;
    sources.push(`$${expression}${malformed ? "" : "$"}`);
  }
  return sources;
}

function parseRequiredCoverage(
  value: unknown,
): Readonly<Record<NotationCoverageFacet, readonly string[]>> {
  const item = record(value, "notation corpus.requiredCoverage");
  exactKeys(item, [...NOTATION_COVERAGE_FACETS], "notation corpus.requiredCoverage");
  const facet = (name: NotationCoverageFacet): readonly string[] => {
    const values = strings(item[name], `notation corpus.requiredCoverage.${name}`);
    if (values.length === 0) fail(`notation corpus.requiredCoverage.${name}`, "empty");
    unique(values, `notation corpus.requiredCoverage.${name}`);
    return values;
  };
  return {
    structuralFamily: facet("structuralFamily"),
    surface: facet("surface"),
    context: facet("context"),
    cursorTarget: facet("cursorTarget"),
    outcome: facet("outcome"),
  };
}

function parseCase(value: unknown, path: string): NotationConformanceCase {
  const item = record(value, path);
  exactKeys(
    item,
    [
      "id",
      "language",
      "content",
      "cursor",
      "expectedAncestor",
      "forbiddenAncestor",
      "coverage",
    ],
    path,
    ["forbiddenAncestor"],
  );
  const language = text(item.language, `${path}.language`);
  if (language !== "latex" && language !== "markdown") {
    fail(`${path}.language`, "must be latex or markdown");
  }
  const cursor = record(item.cursor, `${path}.cursor`);
  exactKeys(cursor, ["needle", "offset"], `${path}.cursor`);
  const needle = text(cursor.needle, `${path}.cursor.needle`);
  const offset = integer(cursor.offset, `${path}.cursor.offset`);
  if (offset < 0 || offset > needle.length) fail(`${path}.cursor.offset`, "outside needle");
  const content = text(item.content, `${path}.content`);
  if (content.indexOf(needle) < 0 || content.indexOf(needle) !== content.lastIndexOf(needle)) {
    fail(`${path}.cursor.needle`, "must occur exactly once");
  }
  return {
    id: identifier(item.id, `${path}.id`),
    language,
    content,
    cursor: { needle, offset },
    expectedAncestor: parseExpectation(item.expectedAncestor, `${path}.expectedAncestor`),
    ...(item.forbiddenAncestor === undefined
      ? {}
      : {
          forbiddenAncestor: parseExpectation(
            item.forbiddenAncestor,
            `${path}.forbiddenAncestor`,
          ),
        }),
    coverage: parseCoverage(item.coverage, `${path}.coverage`),
  };
}

function parseExpectation(value: unknown, path: string): NotationNodeExpectation {
  const item = record(value, path);
  exactKeys(
    item,
    ["kind", "name", "text", "state", "provenanceOrigin"],
    path,
    ["name", "text", "state", "provenanceOrigin"],
  );
  return {
    kind: enumText(item.kind, `${path}.kind`, NOTATION_NODE_KINDS),
    ...(item.name === undefined ? {} : { name: text(item.name, `${path}.name`) }),
    ...(item.text === undefined ? {} : { text: text(item.text, `${path}.text`) }),
    ...(item.state === undefined
      ? {}
      : { state: enumText(item.state, `${path}.state`, NOTATION_STATES) }),
    ...(item.provenanceOrigin === undefined
      ? {}
      : {
          provenanceOrigin: enumText(
            item.provenanceOrigin,
            `${path}.provenanceOrigin`,
            PROVENANCE_ORIGINS,
          ),
        }),
  };
}

function parseCoverage(
  value: unknown,
  path: string,
): Readonly<Record<NotationCoverageFacet, string>> {
  const item = record(value, path);
  exactKeys(item, [...NOTATION_COVERAGE_FACETS], path);
  return {
    structuralFamily: text(item.structuralFamily, `${path}.structuralFamily`),
    surface: text(item.surface, `${path}.surface`),
    context: text(item.context, `${path}.context`),
    cursorTarget: text(item.cursorTarget, `${path}.cursorTarget`),
    outcome: text(item.outcome, `${path}.outcome`),
  };
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(path, "must be an object");
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) fail(path, "must be an array");
  return value;
}

function strings(value: unknown, path: string): string[] {
  return array(value, path).map((item, index) => text(item, `${path}[${index}]`));
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || value.length === 0) fail(path, "must be non-empty text");
  return value;
}

function enumText<const Values extends readonly string[]>(
  value: unknown,
  path: string,
  allowed: Values,
): Values[number] {
  const result = text(value, path);
  if (!allowed.includes(result)) fail(path, `must be one of ${allowed.join(", ")}`);
  return result as Values[number];
}

function identifier(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(result)) fail(path, "must be a kebab identifier");
  return result;
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value)) fail(path, "must be a safe integer");
  return value as number;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) fail(path, "must be unique");
}

function exactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
  optional: readonly string[] = [],
): void {
  const actual = Object.keys(value).sort();
  const required = allowed.filter((key) => !optional.includes(key));
  for (const key of required) if (!(key in value)) fail(path, `missing ${key}`);
  for (const key of actual) if (!allowed.includes(key)) fail(path, `unknown ${key}`);
}

function fail(path: string, message: string): never {
  throw new Error(`${path}: ${message}`);
}
