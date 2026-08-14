import {
  FIRST_LOSS_STAGES,
  type AuthoredScientificFixture,
  type AuthoredScientificProbe,
  type FirstLossStage,
  type ScientificDecision,
} from "./authored-scientific";
import type { AuthoredFirstLossRecord } from "./authored-first-loss";

export const STEM_BREADTH_FIELDS = [
  "shared-foundations",
  "linear-algebra",
  "differential-equations",
  "probability-statistics",
  "numerical-analysis",
] as const;

export const STEM_BREADTH_CAPABILITIES = [
  "vocabulary",
  "typing",
  "relation-recognition",
  "equivalent-forms",
  "conditions",
  "document-attachment",
  "project-lifecycle",
  "decision-quality",
  "navigation",
  "refusal",
] as const;

export type StemBreadthField = (typeof STEM_BREADTH_FIELDS)[number];
export type StemBreadthCapability =
  (typeof STEM_BREADTH_CAPABILITIES)[number];

export interface StemBreadthCell {
  readonly capability: StemBreadthCapability;
  readonly plannedIssue?: string;
  readonly probeIds: readonly string[];
  readonly requirement: string;
  readonly status: "measured" | "commissioned-gap";
}

export interface StemBreadthFieldPlan {
  readonly capabilities: readonly StemBreadthCell[];
  readonly id: StemBreadthField;
  readonly plannedIssue: string;
}

export interface StemBreadthManifest {
  readonly baseline: {
    readonly commit: string;
    readonly developmentFixtureId: string;
    readonly note: string;
    readonly protocolVersion: number;
  };
  readonly fields: readonly StemBreadthFieldPlan[];
  readonly id: string;
  readonly releaseCommission: {
    readonly authoringMethod: "isolated-authors";
    readonly criticMethod: "independent-critics";
    readonly engineExecutionsBeforeSeal: 0;
    readonly historicalFixturesAreTuningInputs: false;
    readonly receiptPolicy: "new-path-one-shot";
    readonly status: "commissioned-unwritten";
  };
  readonly schemaVersion: 1;
  readonly sourcePolicy: {
    readonly developmentFixturePath: string;
    readonly importedText: false;
    readonly provenance: "project-original-independent-review";
  };
}

export interface StemBreadthValidationSummary {
  readonly commissionedGaps: number;
  readonly fields: Readonly<
    Record<
      StemBreadthField,
      { readonly gaps: number; readonly measuredCapabilities: number }
    >
  >;
  readonly measuredCells: number;
  readonly referencedProbes: number;
}

export interface StemBreadthScore {
  readonly capabilities: Readonly<
    Record<StemBreadthCapability, StemBreadthScoreCount>
  >;
  readonly cells: readonly (StemBreadthScoreCount & {
    readonly capability: StemBreadthCapability;
    readonly field: StemBreadthField;
    readonly status: StemBreadthCell["status"];
  })[];
  readonly fields: Readonly<Record<StemBreadthField, StemBreadthScoreCount>>;
  readonly uniqueProbes: StemBreadthScoreCount;
}

export interface StemBreadthScoreCount {
  readonly cases: number;
  readonly passed: number;
}

const CAPABILITY_STAGE = {
  vocabulary: "attachment",
  typing: "typed-fact",
  "relation-recognition": "pack-unification",
  "equivalent-forms": "pack-unification",
  conditions: "typed-fact",
  "document-attachment": "attachment",
  "project-lifecycle": "propagation",
  "decision-quality": "decision",
  navigation: "host-projection",
  refusal: "decision",
} as const satisfies Readonly<Record<StemBreadthCapability, FirstLossStage>>;

export function parseStemBreadthManifest(
  value: unknown,
): StemBreadthManifest {
  const root = record(value, "stem breadth manifest");
  exact(
    root,
    [
      "schemaVersion",
      "id",
      "baseline",
      "sourcePolicy",
      "releaseCommission",
      "fields",
    ],
    "stem breadth manifest",
  );
  if (root.schemaVersion !== 1) {
    throw new Error("stem breadth manifest.schemaVersion: must be 1");
  }
  const baseline = record(root.baseline, "stem breadth manifest.baseline");
  exact(
    baseline,
    ["commit", "protocolVersion", "developmentFixtureId", "note"],
    "stem breadth manifest.baseline",
  );
  const sourcePolicy = record(
    root.sourcePolicy,
    "stem breadth manifest.sourcePolicy",
  );
  exact(
    sourcePolicy,
    ["developmentFixturePath", "provenance", "importedText"],
    "stem breadth manifest.sourcePolicy",
  );
  const releaseCommission = record(
    root.releaseCommission,
    "stem breadth manifest.releaseCommission",
  );
  exact(
    releaseCommission,
    [
      "status",
      "authoringMethod",
      "criticMethod",
      "engineExecutionsBeforeSeal",
      "historicalFixturesAreTuningInputs",
      "receiptPolicy",
    ],
    "stem breadth manifest.releaseCommission",
  );
  const fields = array(root.fields, "stem breadth manifest.fields").map(
    (value, index): StemBreadthFieldPlan => {
      const path = `stem breadth manifest.fields[${index}]`;
      const field = record(value, path);
      exact(field, ["id", "plannedIssue", "capabilities"], path);
      return {
        capabilities: array(field.capabilities, `${path}.capabilities`).map(
          (candidate, capabilityIndex) =>
            parseCell(candidate, `${path}.capabilities[${capabilityIndex}]`),
        ),
        id: oneOf(field.id, STEM_BREADTH_FIELDS, `${path}.id`),
        plannedIssue: issueUrl(field.plannedIssue, `${path}.plannedIssue`),
      };
    },
  );
  return {
    baseline: {
      commit: sha(baseline.commit, "stem breadth manifest.baseline.commit"),
      developmentFixtureId: text(
        baseline.developmentFixtureId,
        "stem breadth manifest.baseline.developmentFixtureId",
      ),
      note: text(baseline.note, "stem breadth manifest.baseline.note"),
      protocolVersion: integer(
        baseline.protocolVersion,
        "stem breadth manifest.baseline.protocolVersion",
      ),
    },
    fields,
    id: text(root.id, "stem breadth manifest.id"),
    releaseCommission: {
      authoringMethod: literal(
        releaseCommission.authoringMethod,
        "isolated-authors",
        "stem breadth manifest.releaseCommission.authoringMethod",
      ),
      criticMethod: literal(
        releaseCommission.criticMethod,
        "independent-critics",
        "stem breadth manifest.releaseCommission.criticMethod",
      ),
      engineExecutionsBeforeSeal: literal(
        releaseCommission.engineExecutionsBeforeSeal,
        0,
        "stem breadth manifest.releaseCommission.engineExecutionsBeforeSeal",
      ),
      historicalFixturesAreTuningInputs: literal(
        releaseCommission.historicalFixturesAreTuningInputs,
        false,
        "stem breadth manifest.releaseCommission.historicalFixturesAreTuningInputs",
      ),
      receiptPolicy: literal(
        releaseCommission.receiptPolicy,
        "new-path-one-shot",
        "stem breadth manifest.releaseCommission.receiptPolicy",
      ),
      status: literal(
        releaseCommission.status,
        "commissioned-unwritten",
        "stem breadth manifest.releaseCommission.status",
      ),
    },
    schemaVersion: 1,
    sourcePolicy: {
      developmentFixturePath: text(
        sourcePolicy.developmentFixturePath,
        "stem breadth manifest.sourcePolicy.developmentFixturePath",
      ),
      importedText: literal(
        sourcePolicy.importedText,
        false,
        "stem breadth manifest.sourcePolicy.importedText",
      ),
      provenance: literal(
        sourcePolicy.provenance,
        "project-original-independent-review",
        "stem breadth manifest.sourcePolicy.provenance",
      ),
    },
  };
}

export function validateStemBreadthBenchmark(
  manifest: StemBreadthManifest,
  fixture: AuthoredScientificFixture,
): StemBreadthValidationSummary {
  if (
    manifest.sourcePolicy.developmentFixturePath !==
    "fixtures/challenge/document-reasoning-development-v1.json"
  ) {
    throw new Error(
      "stem breadth benchmark must use the reviewed public-development fixture",
    );
  }
  if (fixture.batch.split !== "development") {
    throw new Error("stem breadth benchmark requires the editable development fixture");
  }
  if (fixture.batch.id !== manifest.baseline.developmentFixtureId) {
    throw new Error("stem breadth benchmark development fixture id differs");
  }
  exactSet(
    manifest.fields.map((field) => field.id),
    STEM_BREADTH_FIELDS,
    "stem breadth fields",
  );
  const probes = new Map(fixture.probes.map((probe) => [probe.id, probe]));
  const scenarios = new Map(
    fixture.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const referenced = new Set<string>();
  const decisions = new Set<ScientificDecision>();
  const measuredCapabilityFields = new Map<StemBreadthCapability, number>();
  const fields = {} as Record<
    StemBreadthField,
    { gaps: number; measuredCapabilities: number }
  >;
  let commissionedGaps = 0;
  let measuredCells = 0;
  for (const field of manifest.fields) {
    exactSet(
      field.capabilities.map((cell) => cell.capability),
      STEM_BREADTH_CAPABILITIES,
      `${field.id} capabilities`,
    );
    let gaps = 0;
    let measuredCapabilities = 0;
    for (const cell of field.capabilities) {
      if (cell.status === "measured") {
        if (cell.probeIds.length === 0 || cell.plannedIssue !== undefined) {
          throw new Error(
            `${field.id}/${cell.capability}: measured cells require probes and no gap issue`,
          );
        }
        measuredCells += 1;
        measuredCapabilities += 1;
        measuredCapabilityFields.set(
          cell.capability,
          (measuredCapabilityFields.get(cell.capability) ?? 0) + 1,
        );
      } else {
        if (cell.probeIds.length !== 0 || cell.plannedIssue === undefined) {
          throw new Error(
            `${field.id}/${cell.capability}: commissioned gaps require one issue and no probes`,
          );
        }
        gaps += 1;
        commissionedGaps += 1;
      }
      unique(cell.probeIds, `${field.id}/${cell.capability} probe ids`);
      for (const probeId of cell.probeIds) {
        const probe = probes.get(probeId);
        if (!probe) {
          throw new Error(
            `${field.id}/${cell.capability}: unknown development probe ${probeId}`,
          );
        }
        validateReviewProvenance(probe, scenarios);
        referenced.add(probeId);
        decisions.add(probe.expected.decision);
      }
    }
    if (measuredCapabilities < 8) {
      throw new Error(
        `${field.id}: requires at least 8 measured capability cells`,
      );
    }
    fields[field.id] = { gaps, measuredCapabilities };
  }
  for (const capability of STEM_BREADTH_CAPABILITIES) {
    if ((measuredCapabilityFields.get(capability) ?? 0) < 3) {
      throw new Error(
        `${capability}: requires measured evidence in at least three fields`,
      );
    }
  }
  exactSet(
    [...decisions],
    ["established", "partial", "ambiguous", "conflicting", "unsupported"],
    "stem breadth reviewed decisions",
  );
  return {
    commissionedGaps,
    fields,
    measuredCells,
    referencedProbes: referenced.size,
  };
}

export function scoreStemBreadth(
  manifest: StemBreadthManifest,
  records: readonly AuthoredFirstLossRecord[],
): StemBreadthScore {
  const byId = new Map<string, AuthoredFirstLossRecord>();
  for (const record of records) {
    if (byId.has(record.caseId)) {
      throw new Error(`${record.caseId}: duplicate stem breadth observation`);
    }
    byId.set(record.caseId, record);
  }
  const cells = manifest.fields.flatMap((field) =>
    field.capabilities.map((cell) => {
      const count = scoreCell(cell, byId);
      return {
        ...count,
        capability: cell.capability,
        field: field.id,
        status: cell.status,
      };
    }),
  );
  const uniqueProbeIds = new Set(
    manifest.fields.flatMap((field) =>
      field.capabilities.flatMap((cell) => cell.probeIds),
    ),
  );
  const uniqueProbes = [...uniqueProbeIds].reduce<StemBreadthScoreCount>(
    (count, probeId) => {
      const record = requiredRecord(byId, probeId);
      return {
        cases: count.cases + 1,
        passed: count.passed + Number(record.stage === null),
      };
    },
    { cases: 0, passed: 0 },
  );
  return {
    capabilities: Object.fromEntries(
      STEM_BREADTH_CAPABILITIES.map((capability) => [
        capability,
        sum(cells.filter((cell) => cell.capability === capability)),
      ]),
    ) as Record<StemBreadthCapability, StemBreadthScoreCount>,
    cells,
    fields: Object.fromEntries(
      STEM_BREADTH_FIELDS.map((field) => [
        field,
        sum(cells.filter((cell) => cell.field === field)),
      ]),
    ) as Record<StemBreadthField, StemBreadthScoreCount>,
    uniqueProbes,
  };
}

function scoreCell(
  cell: StemBreadthCell,
  records: ReadonlyMap<string, AuthoredFirstLossRecord>,
): StemBreadthScoreCount {
  const target = FIRST_LOSS_STAGES.indexOf(CAPABILITY_STAGE[cell.capability]);
  return cell.probeIds.reduce<StemBreadthScoreCount>(
    (count, probeId) => {
      const record = requiredRecord(records, probeId);
      const loss =
        record.stage === null
          ? Number.POSITIVE_INFINITY
          : FIRST_LOSS_STAGES.indexOf(record.stage);
      return {
        cases: count.cases + 1,
        passed: count.passed + Number(loss > target),
      };
    },
    { cases: 0, passed: 0 },
  );
}

function sum(values: readonly StemBreadthScoreCount[]): StemBreadthScoreCount {
  return values.reduce(
    (total, value) => ({
      cases: total.cases + value.cases,
      passed: total.passed + value.passed,
    }),
    { cases: 0, passed: 0 },
  );
}

function requiredRecord(
  records: ReadonlyMap<string, AuthoredFirstLossRecord>,
  probeId: string,
): AuthoredFirstLossRecord {
  const record = records.get(probeId);
  if (!record) throw new Error(`${probeId}: missing stem breadth observation`);
  return record;
}

function validateReviewProvenance(
  probe: AuthoredScientificProbe,
  scenarios: ReadonlyMap<string, AuthoredScientificFixture["scenarios"][number]>,
): void {
  const scenario = scenarios.get(probe.scenarioId);
  if (!scenario) throw new Error(`${probe.id}: missing reviewed scenario`);
  const reviewers = new Set([
    scenario.provenance.authorId,
    scenario.review.criticId,
    scenario.review.mainReviewer,
  ]);
  if (!scenario.provenance.engineBlind || reviewers.size !== 3) {
    throw new Error(`${probe.id}: benchmark evidence lacks independent review`);
  }
}

function parseCell(value: unknown, path: string): StemBreadthCell {
  const cell = record(value, path);
  exact(
    cell,
    ["capability", "status", "requirement", "probeIds", "plannedIssue"],
    path,
    ["plannedIssue"],
  );
  return {
    capability: oneOf(
      cell.capability,
      STEM_BREADTH_CAPABILITIES,
      `${path}.capability`,
    ),
    ...(cell.plannedIssue === undefined
      ? {}
      : { plannedIssue: issueUrl(cell.plannedIssue, `${path}.plannedIssue`) }),
    probeIds: array(cell.probeIds, `${path}.probeIds`).map((item, index) =>
      text(item, `${path}.probeIds[${index}]`),
    ),
    requirement: text(cell.requirement, `${path}.requirement`),
    status: oneOf(
      cell.status,
      ["measured", "commissioned-gap"] as const,
      `${path}.status`,
    ),
  };
}

function issueUrl(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^https:\/\/github\.com\/corca-ai\/(?:semath|cortex)\/issues\/[1-9][0-9]*$/u.test(result)) {
    throw new Error(`${path}: expected a corca-ai GitHub issue URL`);
  }
  return result;
}

function sha(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^[0-9a-f]{40}$/u.test(result)) {
    throw new Error(`${path}: expected a full Git commit`);
  }
  return result;
}

function exactSet(
  actual: readonly string[],
  expected: readonly string[],
  path: string,
): void {
  unique(actual, path);
  const values = new Set(actual);
  if (actual.length !== expected.length || expected.some((item) => !values.has(item))) {
    throw new Error(`${path}: expected exactly ${expected.join(", ")}`);
  }
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length) {
    throw new Error(`${path}: duplicate value`);
  }
}

function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  path: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  if (unknown) throw new Error(`${path}.${unknown}: unknown field`);
  const optionalKeys = new Set(optional);
  const missing = keys.find(
    (key) => !optionalKeys.has(key) && !(key in value),
  );
  if (missing) throw new Error(`${path}.${missing}: missing field`);
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path}: expected an array`);
  return value;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${path}: expected non-empty text`);
  }
  return value;
}

function integer(value: unknown, path: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) {
    throw new Error(`${path}: expected a non-negative integer`);
  }
  return value as number;
}

function literal<const T extends string | number | boolean>(
  value: unknown,
  expected: T,
  path: string,
): T {
  if (value !== expected) throw new Error(`${path}: expected ${String(expected)}`);
  return expected;
}

function oneOf<const T extends string>(
  value: unknown,
  expected: readonly T[],
  path: string,
): T {
  if (typeof value !== "string" || !expected.includes(value as T)) {
    throw new Error(`${path}: expected one of ${expected.join(", ")}`);
  }
  return value as T;
}
