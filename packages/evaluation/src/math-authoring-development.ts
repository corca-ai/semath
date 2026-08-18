import type { MathAuthoringDisposition } from "../../protocol/src/index";
import {
  compareAuthoredMathAuthoringContext,
  parseAuthoredMathAuthoringExpectation,
  type AuthoredMathAuthoringExpectation,
  type AuthoredScientificSnapshot,
  type ObservedMathAuthoringContext,
} from "./authored-scientific";

export const MATH_AUTHORING_DEVELOPMENT_FEATURES = [
  "approximate-not-exact",
  "capped-engine-limit",
  "conventional-non-authoritative",
  "cross-document-claim-evidence",
  "exact-source-lifecycle",
  "generated-noneditable",
  "revision-fence",
  "retraction",
  "same-entity-notation",
] as const;

export type MathAuthoringDevelopmentFeature =
  (typeof MATH_AUTHORING_DEVELOPMENT_FEATURES)[number];

export interface MathAuthoringDevelopmentDocument {
  readonly content: string;
  readonly fileId: string;
  readonly path: string;
}

export interface MathAuthoringDevelopmentAnchor {
  readonly fileId: string;
  readonly needle: string;
  readonly occurrence?: number;
  readonly selection?: { readonly length: number; readonly offset: number };
}

interface MathAuthoringDevelopmentCaseBase {
  readonly cursor: MathAuthoringDevelopmentAnchor;
  readonly documents: readonly MathAuthoringDevelopmentDocument[];
  readonly expected: {
    readonly authoringContext: AuthoredMathAuthoringExpectation;
    readonly definitionAuthorized: boolean;
  };
  readonly features: readonly MathAuthoringDevelopmentFeature[];
  readonly id: string;
  readonly mainFileId: string;
}

export interface MathAuthoringDevelopmentStaticCase
  extends MathAuthoringDevelopmentCaseBase {
  readonly kind: "static";
}

export interface MathAuthoringDevelopmentRevisionCase
  extends MathAuthoringDevelopmentCaseBase {
  readonly kind: "revision";
  readonly revisedDocuments: readonly MathAuthoringDevelopmentDocument[];
  readonly staleDocumentVersion: number;
}

export type MathAuthoringDevelopmentCase =
  | MathAuthoringDevelopmentRevisionCase
  | MathAuthoringDevelopmentStaticCase;

export interface MathAuthoringDevelopmentFixture {
  readonly cases: readonly MathAuthoringDevelopmentCase[];
  readonly reviewedAt: string;
  readonly reviewedBy: string;
  readonly reviewSummary: string;
  readonly schemaVersion: 1;
}

export interface MathAuthoringDevelopmentObservation {
  readonly caseId: string;
  readonly context: ObservedMathAuthoringContext;
  readonly definitionAuthorized: boolean;
  readonly staleRevisionRejected: boolean;
}

export interface MathAuthoringDevelopmentSummary {
  readonly cases: number;
  readonly coveredFeatures: readonly MathAuthoringDevelopmentFeature[];
}

export function parseMathAuthoringDevelopmentFixture(
  value: unknown,
): MathAuthoringDevelopmentFixture {
  const root = exactRecord(value, "fixture", [
    "cases",
    "reviewedAt",
    "reviewedBy",
    "reviewSummary",
    "schemaVersion",
  ]);
  if (root.schemaVersion !== 1) throw new Error("fixture.schemaVersion must be 1");
  const cases = array(root.cases, "fixture.cases", parseDevelopmentCase);
  const ids = new Set<string>();
  for (const item of cases) {
    if (ids.has(item.id)) throw new Error(`duplicate development case ${item.id}`);
    ids.add(item.id);
  }
  return {
    cases,
    reviewedAt: nonemptyString(root.reviewedAt, "fixture.reviewedAt"),
    reviewedBy: nonemptyString(root.reviewedBy, "fixture.reviewedBy"),
    reviewSummary: nonemptyString(root.reviewSummary, "fixture.reviewSummary"),
    schemaVersion: 1,
  };
}

export function evaluateMathAuthoringDevelopment(
  fixture: MathAuthoringDevelopmentFixture,
  observations: readonly MathAuthoringDevelopmentObservation[],
): MathAuthoringDevelopmentSummary {
  const failures: string[] = [];
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  if (byId.size !== observations.length) failures.push("duplicate observations");
  const covered = new Set<MathAuthoringDevelopmentFeature>();
  for (const item of fixture.cases) {
    const observed = byId.get(item.id);
    if (!observed) {
      failures.push(`${item.id}: missing observation`);
      continue;
    }
    item.features.forEach((feature) => covered.add(feature));
    const comparison = compareAuthoredMathAuthoringContext(
      snapshotFor(item),
      item.expected.authoringContext,
      observed.context,
    );
    failures.push(
      ...comparison.missing.map((message) => `${item.id}: missing ${message}`),
      ...comparison.unexpected.map((message) => `${item.id}: unexpected ${message}`),
      ...comparison.unsafeLifecycle.map(
        (message) => `${item.id}: unsafe lifecycle ${message}`,
      ),
    );
    if (comparison.moreAuthoritativeDisposition) {
      failures.push(`${item.id}: disposition became more authoritative`);
    }
    if (comparison.falseConflictDisposition) {
      failures.push(`${item.id}: disposition introduced a conflict`);
    }
    if (observed.definitionAuthorized !== item.expected.definitionAuthorized) {
      failures.push(
        `${item.id}: definition authority ${observed.definitionAuthorized}; expected ${item.expected.definitionAuthorized}`,
      );
    }
    if (item.kind === "revision" && !observed.staleRevisionRejected) {
      failures.push(`${item.id}: stale document revision was accepted`);
    }
    if (item.kind === "static" && observed.staleRevisionRejected) {
      failures.push(`${item.id}: unexpected stale-revision result`);
    }
  }
  for (const observation of observations) {
    if (!fixture.cases.some((item) => item.id === observation.caseId)) {
      failures.push(`${observation.caseId}: unexpected observation`);
    }
  }
  for (const feature of MATH_AUTHORING_DEVELOPMENT_FEATURES) {
    if (!covered.has(feature)) failures.push(`missing development feature ${feature}`);
  }
  if (failures.length > 0) {
    throw new Error(`math-authoring development evidence failed:\n${failures.join("\n")}`);
  }
  return {
    cases: fixture.cases.length,
    coveredFeatures: MATH_AUTHORING_DEVELOPMENT_FEATURES,
  };
}

function parseDevelopmentCase(
  value: unknown,
  path: string,
): MathAuthoringDevelopmentCase {
  const base = record(value, path);
  const commonKeys = [
    "cursor",
    "documents",
    "expected",
    "features",
    "id",
    "kind",
    "mainFileId",
  ];
  const item = exactRecord(
    base,
    path,
    base.kind === "revision"
      ? [...commonKeys, "revisedDocuments", "staleDocumentVersion"]
      : commonKeys,
  );
  if (item.kind !== "revision" && item.kind !== "static") {
    throw new Error(`${path}.kind must be static or revision`);
  }
  const features = array(item.features, `${path}.features`, (feature, featurePath) => {
    const parsed = nonemptyString(feature, featurePath);
    if (!(MATH_AUTHORING_DEVELOPMENT_FEATURES as readonly string[]).includes(parsed)) {
      throw new Error(`${featurePath}: unknown feature ${parsed}`);
    }
    return parsed as MathAuthoringDevelopmentFeature;
  });
  if (features.length === 0) throw new Error(`${path}.features must not be empty`);
  const expected = exactRecord(item.expected, `${path}.expected`, [
    "authoringContext",
    "definitionAuthorized",
  ]);
  const common = {
    cursor: parseAnchor(item.cursor, `${path}.cursor`),
    documents: array(item.documents, `${path}.documents`, parseDocument),
    expected: {
      authoringContext: parseAuthoredMathAuthoringExpectation(
        expected.authoringContext,
        `${path}.expected.authoringContext`,
      ),
      definitionAuthorized: boolean(
        expected.definitionAuthorized,
        `${path}.expected.definitionAuthorized`,
      ),
    },
    features,
    id: nonemptyString(item.id, `${path}.id`),
    mainFileId: nonemptyString(item.mainFileId, `${path}.mainFileId`),
  };
  if (common.documents.length === 0) throw new Error(`${path}.documents must not be empty`);
  if (item.kind === "static") return { ...common, kind: item.kind };
  const revisedDocuments = array(
    item.revisedDocuments,
    `${path}.revisedDocuments`,
    parseDocument,
  );
  if (revisedDocuments.length === 0) {
    throw new Error(`${path}.revisedDocuments must not be empty`);
  }
  return {
    ...common,
    kind: item.kind,
    revisedDocuments,
    staleDocumentVersion: nonnegativeInteger(
      item.staleDocumentVersion,
      `${path}.staleDocumentVersion`,
    ),
  };
}

function snapshotFor(
  item: MathAuthoringDevelopmentCase,
): AuthoredScientificSnapshot {
  return {
    documents: item.kind === "revision" ? item.revisedDocuments : item.documents,
    id: item.id,
  };
}

function parseDocument(
  value: unknown,
  path: string,
): MathAuthoringDevelopmentDocument {
  const item = exactRecord(value, path, ["content", "fileId", "path"]);
  return {
    content: string(item.content, `${path}.content`),
    fileId: nonemptyString(item.fileId, `${path}.fileId`),
    path: nonemptyString(item.path, `${path}.path`),
  };
}

function parseAnchor(
  value: unknown,
  path: string,
): MathAuthoringDevelopmentAnchor {
  const item = exactRecord(
    value,
    path,
    ["fileId", "needle", "occurrence", "selection"],
    ["occurrence", "selection"],
  );
  return {
    fileId: nonemptyString(item.fileId, `${path}.fileId`),
    needle: nonemptyString(item.needle, `${path}.needle`),
    ...(item.occurrence === undefined
      ? {}
      : { occurrence: nonnegativeInteger(item.occurrence, `${path}.occurrence`) }),
    ...(item.selection === undefined
      ? {}
      : { selection: parseSelection(item.selection, `${path}.selection`) }),
  };
}

function parseSelection(
  value: unknown,
  path: string,
): { readonly length: number; readonly offset: number } {
  const item = exactRecord(value, path, ["length", "offset"]);
  const length = nonnegativeInteger(item.length, `${path}.length`);
  if (length === 0) throw new Error(`${path}.length must be positive`);
  return {
    length,
    offset: nonnegativeInteger(item.offset, `${path}.offset`),
  };
}

function exactRecord(
  value: unknown,
  path: string,
  keys: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> {
  const item = record(value, path);
  const allowed = new Set(keys);
  for (const key of Object.keys(item)) {
    if (!allowed.has(key)) throw new Error(`${path}.${key} is not allowed`);
  }
  for (const key of keys) {
    if (!(key in item) && !optional.includes(key)) {
      throw new Error(`${path}.${key} is required`);
    }
  }
  return item;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function array<T>(
  value: unknown,
  path: string,
  parse: (value: unknown, path: string) => T,
): T[] {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array`);
  return value.map((item, index) => parse(item, `${path}[${index}]`));
}

function string(value: unknown, path: string): string {
  if (typeof value !== "string") throw new Error(`${path} must be a string`);
  return value;
}

function nonemptyString(value: unknown, path: string): string {
  const parsed = string(value, path);
  if (parsed.length === 0) throw new Error(`${path} must not be empty`);
  return parsed;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path} must be a boolean`);
  return value;
}

function nonnegativeInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) {
    throw new Error(`${path} must be a non-negative integer`);
  }
  return value as number;
}
