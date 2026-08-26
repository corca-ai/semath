import type {
  MathAuthoringContext,
  MathInterpretationHypothesisInfo,
  SourceRange,
} from "../../protocol/src/index";
import {
  authoredScenarioFor,
  authoredSnapshotFor,
  resolveAuthoredAnchor,
  type AuthoredScientificObservation,
  type AuthoredSourceAnchor,
} from "./authored-scientific";
import type { FreshBlindReleaseFixture } from "./fresh-blind-release";
import {
  mathAuthoringContextSafetyFailures,
  type MathAuthoringContextFailure,
} from "./math-authoring-development";
import { hypothesisIsMathematicalAuthority } from "./challenge-observation";

export interface FreshBlindAuthoringHypothesisSelector {
  readonly anchor: AuthoredSourceAnchor;
  readonly kind: MathInterpretationHypothesisInfo["kind"];
  readonly relationId: string | null;
}

export interface FreshBlindAuthoringSafetyExpectation {
  readonly allowedAuthority: readonly FreshBlindAuthoringHypothesisSelector[];
  readonly allowedContradictions: readonly FreshBlindAuthoringHypothesisSelector[];
  readonly forbiddenDispositions: readonly MathAuthoringContext["disposition"][];
  readonly lifecycle: Pick<
    MathAuthoringContext["lifecycle"],
    "capped" | "editable" | "engineLimited" | "generation" | "retracted"
  >;
  readonly probeId: string;
  readonly requiredAuthority: readonly FreshBlindAuthoringHypothesisSelector[];
  readonly requiredContradictions: readonly FreshBlindAuthoringHypothesisSelector[];
}

export interface FreshBlindAuthoringSafetySummary {
  readonly cases: number;
  readonly failures: readonly MathAuthoringContextFailure[];
}

export function parseFreshBlindAuthoringSafety(
  value: unknown,
  path: string,
): readonly FreshBlindAuthoringSafetyExpectation[] {
  return array(value, path).map((entry, index) => {
    const itemPath = `${path}[${index}]`;
    const item = object(entry, itemPath, [
      "allowedAuthority",
      "allowedContradictions",
      "forbiddenDispositions",
      "lifecycle",
      "probeId",
      "requiredAuthority",
      "requiredContradictions",
    ]);
    const forbiddenDispositions = array(
      item.forbiddenDispositions,
      `${itemPath}.forbiddenDispositions`,
    ).map((disposition, dispositionIndex) =>
      choice(
        disposition,
        [
          "established",
          "partial",
          "ambiguous",
          "conflicting",
          "conventional",
          "engine-limited",
          "unsupported",
        ] as const,
        `${itemPath}.forbiddenDispositions[${dispositionIndex}]`,
      )
    );
    sortedUnique(
      forbiddenDispositions,
      `${itemPath}.forbiddenDispositions`,
    );
    const lifecyclePath = `${itemPath}.lifecycle`;
    const lifecycle = object(item.lifecycle, lifecyclePath, [
      "capped",
      "editable",
      "engineLimited",
      "generation",
      "retracted",
    ]);
    return {
      allowedAuthority: selectors(
        item.allowedAuthority,
        `${itemPath}.allowedAuthority`,
      ),
      allowedContradictions: selectors(
        item.allowedContradictions,
        `${itemPath}.allowedContradictions`,
      ),
      forbiddenDispositions,
      lifecycle: {
        capped: bool(lifecycle.capped, `${lifecyclePath}.capped`),
        editable: bool(lifecycle.editable, `${lifecyclePath}.editable`),
        engineLimited: bool(
          lifecycle.engineLimited,
          `${lifecyclePath}.engineLimited`,
        ),
        generation: choice(
          lifecycle.generation,
          ["authored", "generated"] as const,
          `${lifecyclePath}.generation`,
        ),
        retracted: bool(lifecycle.retracted, `${lifecyclePath}.retracted`),
      },
      probeId: text(item.probeId, `${itemPath}.probeId`),
      requiredAuthority: selectors(
        item.requiredAuthority,
        `${itemPath}.requiredAuthority`,
      ),
      requiredContradictions: selectors(
        item.requiredContradictions,
        `${itemPath}.requiredContradictions`,
      ),
    };
  });
}

export function validateFreshAuthoringSafetyExpectations(
  release: FreshBlindReleaseFixture,
): void {
  const expectations = release.authoringSafety ?? [];
  const probeIds = new Set(release.fixture.probes.map((probe) => probe.id));
  if (expectations.length !== probeIds.size) {
    throw new Error(
      "fresh authoring safety contract must cover every primary and breadth probe",
    );
  }
  if (stableJson(expectations.map((item) => item.probeId)) !==
    stableJson(release.fixture.probes.map((probe) => probe.id))) {
    throw new Error(
      "fresh authoring safety contract must follow canonical probe order",
    );
  }
  const seen = new Set<string>();
  for (const expectation of expectations) {
    if (!probeIds.has(expectation.probeId) || seen.has(expectation.probeId)) {
      throw new Error(
        `${expectation.probeId}: unknown or duplicate fresh authoring safety probe`,
      );
    }
    seen.add(expectation.probeId);
    const probe = release.fixture.probes.find(
      (candidate) => candidate.id === expectation.probeId,
    )!;
    const snapshot = authoredSnapshotFor(
      authoredScenarioFor(release.fixture, probe),
      probe,
    );
    for (const [label, values] of selectorSets(expectation)) {
      sortedUnique(values.map(selectorKey), `${expectation.probeId}.${label}`);
      for (const selector of values) resolveAuthoredAnchor(snapshot, selector.anchor);
    }
    assertSubset(
      expectation.probeId,
      "requiredAuthority",
      expectation.requiredAuthority,
      expectation.allowedAuthority,
    );
    assertSubset(
      expectation.probeId,
      "requiredContradictions",
      expectation.requiredContradictions,
      expectation.allowedContradictions,
    );
    if ((expectation.lifecycle.retracted ||
      expectation.lifecycle.generation === "generated") &&
      expectation.lifecycle.editable) {
      throw new Error(
        `${expectation.probeId}.lifecycle: retracted or generated contexts cannot be editable`,
      );
    }
  }
}

export function freshBlindAuthoringSafetySummary(
  release: FreshBlindReleaseFixture,
  observations: readonly AuthoredScientificObservation[],
): FreshBlindAuthoringSafetySummary {
  const expectations = release.authoringSafety ?? [];
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: MathAuthoringContextFailure[] = [];
  for (const expectation of expectations) {
    const context = byId.get(expectation.probeId)?.authoringContext;
    const prefix = `${expectation.probeId}.authoringContext`;
    if (!context) {
      failures.push({ kind: "missing", path: prefix });
      continue;
    }
    failures.push(...mathAuthoringContextSafetyFailures(context).map((item) => ({
      ...item,
      path: `${expectation.probeId}.${item.path}`,
    })));
    compareLifecycle(context, expectation, prefix, failures);
    if (expectation.forbiddenDispositions.includes(context.disposition)) {
      failures.push({
        actual: context.disposition,
        kind: context.disposition === "conflicting"
          ? "false-conflict"
          : context.disposition === "established"
          ? "authority-escalation"
          : "mismatch",
        path: `${prefix}.disposition`,
      });
    }
    const probe = release.fixture.probes.find(
      (candidate) => candidate.id === expectation.probeId,
    )!;
    const snapshot = authoredSnapshotFor(
      authoredScenarioFor(release.fixture, probe),
      probe,
    );
    compareHypotheses(
      context.interpretations.hypotheses.filter(hypothesisIsMathematicalAuthority),
      expectation.allowedAuthority,
      expectation.requiredAuthority,
      snapshot,
      `${prefix}.interpretations.authority`,
      "authority-escalation",
      failures,
    );
    compareHypotheses(
      context.interpretations.hypotheses.filter(
        (hypothesis) => hypothesis.support === "contradicted",
      ),
      expectation.allowedContradictions,
      expectation.requiredContradictions,
      snapshot,
      `${prefix}.interpretations.contradictions`,
      "false-conflict",
      failures,
    );
  }
  return { cases: expectations.length, failures };
}

function compareLifecycle(
  context: MathAuthoringContext,
  expectation: FreshBlindAuthoringSafetyExpectation,
  prefix: string,
  failures: MathAuthoringContextFailure[],
): void {
  if (context.lifecycle.documentVersion !== 1) {
    failures.push({
      actual: context.lifecycle.documentVersion,
      expected: 1,
      kind: "unsafe-lifecycle",
      path: `${prefix}.lifecycle.documentVersion`,
    });
  }
  for (const field of [
    "capped",
    "editable",
    "engineLimited",
    "generation",
    "retracted",
  ] as const) {
    if (context.lifecycle[field] !== expectation.lifecycle[field]) {
      failures.push({
        actual: context.lifecycle[field],
        expected: expectation.lifecycle[field],
        kind: "unsafe-lifecycle",
        path: `${prefix}.lifecycle.${field}`,
      });
    }
  }
}

function compareHypotheses(
  hypotheses: readonly MathInterpretationHypothesisInfo[],
  allowed: readonly FreshBlindAuthoringHypothesisSelector[],
  required: readonly FreshBlindAuthoringHypothesisSelector[],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  path: string,
  unexpectedKind: "authority-escalation" | "false-conflict",
  failures: MathAuthoringContextFailure[],
): void {
  hypotheses.forEach((hypothesis, index) => {
    if (!allowed.some((selector) => matches(hypothesis, selector, snapshot))) {
      failures.push({ actual: hypothesis.kind, kind: unexpectedKind, path: `${path}[${index}]` });
    }
  });
  required.forEach((selector, index) => {
    if (!hypotheses.some((hypothesis) => matches(hypothesis, selector, snapshot))) {
      failures.push({ expected: selector, kind: "missing", path: `${path}.required[${index}]` });
    }
  });
}

function matches(
  hypothesis: MathInterpretationHypothesisInfo,
  selector: FreshBlindAuthoringHypothesisSelector,
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): boolean {
  const anchor = resolveAuthoredAnchor(snapshot, selector.anchor);
  const location = hypothesis.formula?.location ?? hypothesis.location;
  return hypothesis.kind === selector.kind &&
    (hypothesis.relation?.relationId ?? null) === selector.relationId &&
    location.fileId === anchor.fileId && location.path === anchor.path &&
    sameRange(location.range, anchor.range);
}

function selectorSets(expectation: FreshBlindAuthoringSafetyExpectation) {
  return [
    ["allowedAuthority", expectation.allowedAuthority],
    ["requiredAuthority", expectation.requiredAuthority],
    ["allowedContradictions", expectation.allowedContradictions],
    ["requiredContradictions", expectation.requiredContradictions],
  ] as const;
}

function selectors(
  value: unknown,
  path: string,
): readonly FreshBlindAuthoringHypothesisSelector[] {
  return array(value, path).map((entry, index) => {
    const itemPath = `${path}[${index}]`;
    const item = object(entry, itemPath, ["anchor", "kind", "relationId"]);
    return {
      anchor: anchor(item.anchor, `${itemPath}.anchor`),
      kind: choice(
        item.kind,
        ["source-meaning", "typed-law", "scoped-domain", "structural-alternative", "reviewed-convention"] as const,
        `${itemPath}.kind`,
      ),
      relationId: item.relationId === null
        ? null
        : text(item.relationId, `${itemPath}.relationId`),
    };
  });
}

function anchor(value: unknown, path: string): AuthoredSourceAnchor {
  const item = record(value, path);
  const allowed = new Set(["fileId", "needle", "occurrence", "selection"]);
  exact(item, ["fileId", "needle"], path, allowed);
  const needle = text(item.needle, `${path}.needle`);
  const occurrence = item.occurrence === undefined
    ? undefined
    : positive(item.occurrence, `${path}.occurrence`);
  let selection: AuthoredSourceAnchor["selection"];
  if (item.selection !== undefined) {
    const selected = object(item.selection, `${path}.selection`, ["length", "offset"]);
    const length = positive(selected.length, `${path}.selection.length`);
    const offset = nonnegative(selected.offset, `${path}.selection.offset`);
    if (offset + length > needle.length) {
      throw new Error(`${path}.selection: selection exceeds needle`);
    }
    selection = { length, offset };
  }
  return {
    fileId: text(item.fileId, `${path}.fileId`),
    needle,
    ...(occurrence === undefined ? {} : { occurrence }),
    ...(selection === undefined ? {} : { selection }),
  };
}

function assertSubset(
  probeId: string,
  label: string,
  subset: readonly FreshBlindAuthoringHypothesisSelector[],
  superset: readonly FreshBlindAuthoringHypothesisSelector[],
): void {
  const allowed = new Set(superset.map(selectorKey));
  if (subset.some((selector) => !allowed.has(selectorKey(selector)))) {
    throw new Error(`${probeId}.${label}: required selectors must be allowed`);
  }
}

function sortedUnique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length ||
    stableJson(values) !== stableJson([...values].sort())) {
    throw new Error(`${path}: values must be sorted and unique`);
  }
}

function selectorKey(value: FreshBlindAuthoringHypothesisSelector): string {
  return stableJson(value);
}

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return left.startOffset === right.startOffset && left.endOffset === right.endOffset;
}

function array(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path}: expected an array`);
  return value;
}

function object(
  value: unknown,
  path: string,
  required: readonly string[],
): Record<string, unknown> {
  const item = record(value, path);
  exact(item, required, path);
  return item;
}

function exact(
  value: Record<string, unknown>,
  required: readonly string[],
  path: string,
  allowed: ReadonlySet<string> = new Set(required),
): void {
  const missing = required.filter((key) => !(key in value));
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (missing.length || unknown.length) {
    throw new Error(`${path}: unexpected or missing fields`);
  }
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function choice<const T extends readonly string[]>(
  value: unknown,
  options: T,
  path: string,
): T[number] {
  if (typeof value !== "string" || !options.includes(value)) {
    throw new Error(`${path}: expected ${options.join(" or ")}`);
  }
  return value as T[number];
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path}: expected non-empty text`);
  }
  return value;
}

function bool(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path}: expected boolean`);
  return value;
}

function positive(value: unknown, path: string): number {
  const result = nonnegative(value, path);
  if (result < 1) throw new Error(`${path}: expected a positive integer`);
  return result;
}

function nonnegative(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw new Error(`${path}: expected a nonnegative integer`);
  }
  return value;
}

function stableJson(value: unknown): string {
  return JSON.stringify(sortJson(value));
}

function sortJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortJson);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, child]) => [key, sortJson(child)]),
  );
}
