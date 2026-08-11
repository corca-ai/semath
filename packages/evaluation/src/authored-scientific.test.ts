import { describe, expect, test } from "bun:test";
import {
  AUTHORED_AREA_ALLOCATION,
  DOCUMENT_REASONING_FAMILIES,
  authoredFixtureSealPayload,
  authoredScenarioReviewPayload,
  parseAuthoredScientificFixture,
  scoreAuthoredScientificFixture,
  validateAuthoredScientificTranche,
  type AuthoredArea,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredSplit,
  type ScientificDecision,
} from "./authored-scientific";

describe("independently authored scientific corpus", () => {
  test("keeps lifecycle snapshots explicit and cursors on unique source", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    expect(fixture.scenarios[0]?.snapshots.map((snapshot) => snapshot.id)).toEqual([
      "stage-1",
    ]);
    const broken = fixtureValue("holdout", 1) as FixtureValue;
    broken.probes[0]!.cursor.needle = "missing";
    expect(() => parseAuthoredScientificFixture(broken)).toThrow(
      "anchor needle must identify exactly one occurrence",
    );
  });

  test("selects a reviewed occurrence when exact math repeats", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    value.scenarios[0]!.snapshots[0]!.documents[0]!.content +=
      " The independent scope repeats $x_0=y_0$.";
    value.probes[0]!.cursor.occurrence = 1;
    expect(parseAuthoredScientificFixture(value).probes[0]?.cursor.occurrence).toBe(1);

    delete value.probes[0]!.cursor.occurrence;
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "anchor needle must identify exactly one occurrence",
    );
  });

  test("selects an exact symbol range inside a stable source anchor", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    const needle = "unique relation $x_0=y_0$";
    value.probes[0]!.expected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [
        {
          fileId: "main",
          needle,
          selection: { length: 3, offset: needle.indexOf("x_0") },
        },
      ],
      status: "available",
    };
    expect(
      parseAuthoredScientificFixture(value).probes[0]?.expected.navigation
        .definition.required[0]?.selection,
    ).toEqual({ length: 3, offset: needle.indexOf("x_0") });

    value.probes[0]!.expected.navigation.definition.required[0]!.selection = {
      length: needle.length,
      offset: 1,
    };
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "selection must fall within the anchor needle",
    );
  });

  test("rejects same-document relations grounded after the cursor", () => {
    const value = fixtureValue("development", 1) as FixtureValue;
    value.scenarios[0]!.snapshots[0]!.documents[0]!.content +=
      " The later result is $u_0=v_0$.";
    value.probes[0]!.expected.relations = [
      {
        anchor: { fileId: "main", needle: "$u_0=v_0$" },
        relationId: "later-result",
        roles: [
          { role: "left", symbol: "u_0" },
          { role: "right", symbol: "v_0" },
        ],
        sourceGrounded: true,
      },
    ];
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "relation anchor occurs after the cursor evidence boundary",
    );
  });

  test("freezes holdout evidence without coupling it to a baseline run", () => {
    const holdout = fixtureValue("holdout", 1) as FixtureValue;
    delete holdout.scenarios[0]!.review.frozenAt;
    expect(() => parseAuthoredScientificFixture(holdout)).toThrow(
      "holdout review must be frozen",
    );

    const development = fixtureValue("development", 1) as FixtureValue;
    development.scenarios[0]!.review.frozenAt = "2026-08-12T00:00:00Z";
    expect(() => parseAuthoredScientificFixture(development)).toThrow(
      "development scenario must remain editable",
    );
  });

  test("validates exact allocation, decision breadth, law breadth, and split isolation", () => {
    const decisions: ScientificDecision[] = [
      ...Array<ScientificDecision>(56).fill("established"),
      ...Array<ScientificDecision>(36).fill("partial"),
      ...Array<ScientificDecision>(24).fill("ambiguous"),
      ...Array<ScientificDecision>(16).fill("conflicting"),
      ...Array<ScientificDecision>(12).fill("unsupported"),
    ];
    const development = parseAuthoredScientificFixture(
      trancheValue("development", decisions.slice(0, 96)),
    );
    const holdout = parseAuthoredScientificFixture(
      trancheValue("holdout", decisions.slice(96)),
    );
    const summary = validateAuthoredScientificTranche(development, holdout, [
      {
        field: "electromagnetism",
        lawId: "electromagnetism:test-law",
        roles: [
          { id: "left", variadic: false },
          { id: "right", variadic: false },
        ],
      },
    ], ["electromagnetism"]);
    expect(summary.developmentCases).toBe(96);
    expect(summary.holdoutCases).toBe(48);
    expect(summary.decisions).toEqual({
      ambiguous: 24,
      conflicting: 16,
      established: 56,
      partial: 36,
      unsupported: 12,
    });
    expect(Object.values(summary.holdoutFamilies)).toEqual([8, 8, 8, 8, 8, 8]);
  });

  test("scores unsafe conclusions and exact source paths above missed coverage", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    value.probes[0]!.expected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [{ fileId: "main", needle: "$x_0=y_0$" }],
      status: "available",
    };
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.decision = "established";
    observation.proofGrounded = true;
    const startOffset = "Case 0 defines the unique relation ".length;
    observation.definitions = [
      {
        fileId: "main",
        path: "other.tex",
        range: { startOffset, endOffset: startOffset + "$x_0=y_0$".length },
      },
    ];
    const score = scoreAuthoredScientificFixture(fixture, [observation]);
    expect(score.risk.falseEstablishment).toBe(1);
    expect(score.risk.navigationOrIdentity).toBe(1);
    expect(score.risk.total).toBeGreaterThan(score.risk.missedCoverage * 2);
  });

  test("review and seal payloads exclude only their own attestations", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    expect(authoredScenarioReviewPayload(fixture, "scenario-0")).not.toContain(
      "semanticReviewDigest",
    );
    expect(authoredScenarioReviewPayload(fixture, "scenario-0")).toContain(
      "proofGrounded",
    );
    expect(authoredFixtureSealPayload(fixture)).not.toContain(fixture.batch.seal!);
  });

  test("review and seal payloads do not depend on JSON key order", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    const scenario = fixture.scenarios[0]!;
    const reorderedScenario = Object.fromEntries(
      Object.entries(scenario).reverse(),
    ) as unknown as typeof scenario;
    const reordered = {
      ...fixture,
      batch: Object.fromEntries(
        Object.entries(fixture.batch).reverse(),
      ),
      scenarios: [reorderedScenario],
    } as unknown as AuthoredScientificFixture;
    expect(authoredScenarioReviewPayload(reordered, scenario.id)).toBe(
      authoredScenarioReviewPayload(fixture, scenario.id),
    );
    expect(authoredFixtureSealPayload(reordered)).toBe(
      authoredFixtureSealPayload(fixture),
    );
  });
});

function trancheValue(
  split: AuthoredSplit,
  decisions: readonly ScientificDecision[],
): unknown {
  const values: unknown[] = [];
  let decisionIndex = split === "development" ? 0 : 96;
  for (const [field, allocation] of Object.entries(AUTHORED_AREA_ALLOCATION) as [
    AuthoredArea,
    { development: number; holdout: number },
  ][]) {
    for (let index = 0; index < allocation[split]; index += 1) {
      const value = fixtureValue(split, 1, decisionIndex, [decisions[decisionIndex - (split === "development" ? 0 : 96)]!], field) as FixtureValue;
      values.push(value);
      decisionIndex += 1;
    }
  }
  const fixtures = values as FixtureValue[];
  const scenarios = fixtures.flatMap((fixture) => fixture.scenarios);
  const probes = fixtures.flatMap((fixture) => fixture.probes);
  if (split === "holdout") {
    probes.forEach((probe, index) => {
      probe.family = DOCUMENT_REASONING_FAMILIES[Math.floor(index / 8)]!;
    });
  }
  const electromagnetic = scenarios.filter(
    (scenario) => scenario.field === "electromagnetism",
  );
  electromagnetic.forEach((scenario, index) => {
    scenario.lawIds = ["electromagnetism:test-law"];
    scenario.genre = index % 2 === 0 ? "lab-note" : "design-memo";
  });
  for (const probe of probes.filter((probe) =>
    electromagnetic.some((scenario) => scenario.id === probe.scenarioId),
  )) {
    probe.expected.relations = [
      {
        anchor: { fileId: "main", needle: probe.cursor.needle },
        relationId: "electromagnetism:test-law",
        roles: [
          { role: "left", symbol: "x" },
          { role: "right", symbol: "y" },
        ],
        sourceGrounded: true,
      },
    ];
  }
  return {
    batch: batchValue(split),
    probes,
    scenarios,
    schemaVersion: 1,
  };
}

function fixtureValue(
  split: AuthoredSplit,
  count: number,
  start = 0,
  decisions: readonly ScientificDecision[] = Array<ScientificDecision>(count).fill(
    "partial",
  ),
  field: AuthoredArea = "cross-field",
): unknown {
  const scenarios = Array.from({ length: count }, (_, localIndex) => {
    const index = start + localIndex;
    const finalDigest = hex(index + 3000);
    return {
      field,
      genre: index % 2 ? "lab-note" : "design-memo",
      id: `scenario-${index}`,
      lawIds: [],
      provenance: {
        authorId: `author-${index}`,
        engineBlind: true,
        independenceGroup: `${split}-${index}`,
        rawDigest: hex(index + 2000),
        taskCardDigest: hex(index + 1000),
      },
      review: {
        correctionSummary: [],
        criticId: `critic-${index}`,
        finalDigest,
        ...(split === "holdout" ? { frozenAt: "2026-08-12T00:00:00Z" } : {}),
        mainReviewer: "main-codex",
        reviewedAt: "2026-08-12",
        semanticReviewDigest: finalDigest,
        status: "approved",
      },
      snapshots: [
        {
          documents: [
            {
              content: `Case ${index} defines the unique relation $x_${index}=y_${index}$.`,
              fileId: "main",
              path: "main.tex",
            },
          ],
          id: "stage-1",
        },
      ],
      variationTags: ["document-shaped", `case-${index}`],
    };
  });
  return {
    batch: batchValue(split),
    probes: scenarios.map((scenario, localIndex) => ({
      cursor: {
        edge: "after",
        fileId: "main",
        needle: `$x_${start + localIndex}=y_${start + localIndex}$`,
        snapshotId: "stage-1",
      },
      expected: {
        decision: decisions[localIndex],
        diagnostics: { excludedCodes: [], maximum: 0, required: [] },
        excludedRelationIds: [],
        navigation: {
          definition: { excluded: [], minimum: 0, required: [], status: "unavailable" },
          prepareRename: { status: "unavailable" },
          references: { excluded: [], minimum: 0, required: [], status: "unavailable" },
          rename: { excluded: [], minimum: 0, required: [], status: "unavailable" },
        },
        proofGrounded: false,
        relations: [],
        symbol: `x_${start + localIndex}`,
      },
      family: DOCUMENT_REASONING_FAMILIES[localIndex % DOCUMENT_REASONING_FAMILIES.length],
      id: `probe-${start + localIndex}`,
      kind: "primary",
      scenarioId: scenario.id,
    })),
    scenarios,
    schemaVersion: 1,
  };
}

function batchValue(split: AuthoredSplit): Record<string, unknown> {
  return {
    createdAt: "2026-08-12",
    ...(split === "holdout"
      ? { frozenAt: "2026-08-12T00:00:00Z", seal: "d".repeat(64) }
      : {}),
    id: `${split}-batch`,
    reviewPolicyVersion: 1,
    split,
    taskCardDigest: split === "holdout" ? "b".repeat(64) : "c".repeat(64),
  };
}

function observationValue(): WritableObservation {
  return {
    caseId: "probe-0",
    decision: "partial",
    definitions: [],
    diagnostics: [],
    prepareRename: {},
    proofGrounded: false,
    references: [],
    relations: [],
    renameEdits: [],
    symbol: "x_0",
  };
}

function hex(value: number): string {
  return value.toString(16).padStart(64, "0");
}

type WritableObservation = {
  -readonly [Key in keyof AuthoredScientificObservation]: AuthoredScientificObservation[Key];
};

interface FixtureValue {
  batch: Record<string, unknown>;
  probes: {
    cursor: { needle: string; occurrence?: number };
    expected: {
      navigation: {
        definition: {
          excluded: { fileId: string; needle: string }[];
          minimum: number;
          required: {
            fileId: string;
            needle: string;
            selection?: { length: number; offset: number };
          }[];
          status: "available" | "unavailable";
        };
      };
      relations: {
        anchor: { fileId: string; needle: string };
        relationId: string;
        roles: { role: string; symbol: string }[];
        sourceGrounded: boolean;
      }[];
    };
    family: (typeof DOCUMENT_REASONING_FAMILIES)[number];
    scenarioId: string;
  }[];
  scenarios: {
    field: AuthoredArea;
    genre: string;
    id: string;
    lawIds: string[];
    review: { frozenAt?: string };
    snapshots: {
      documents: { content: string }[];
    }[];
  }[];
}
