import { describe, expect, test } from "bun:test";
import {
  planSemanticSafetySuite,
  resolveSemanticSafetyAnchor,
  scoreSemanticSafetySuite,
  semanticSafetyCursorOffset,
  semanticSafetyTransformApplicable,
  SEMANTIC_SAFETY_CONTRACTS,
  type PlannedSemanticSafetyCase,
  type SemanticSafetyObservation,
} from "./semantic-safety";
import { loadSemanticSafetySpec } from "../../../scripts/check-semantic-safety-fixture";

describe("v0.30 open semantic safety planning", () => {
  test("materializes every reviewed contract and metamorphic variant deterministically", async () => {
    const spec = await loadSemanticSafetySpec();
    const before = JSON.stringify(spec);
    const first = planSemanticSafetySuite(spec);
    expect(first).toEqual(planSemanticSafetySuite(spec));
    expect(first).toHaveLength(39);
    expect(new Set(first.map((item) => item.contract))).toEqual(
      new Set(SEMANTIC_SAFETY_CONTRACTS),
    );
    expect(first.filter((item) => item.transform !== "identity")).toHaveLength(26);
    expect(JSON.stringify(spec)).toBe(before);
  });

  test("rejects false establishment and incomplete rename sets", async () => {
    const spec = await loadSemanticSafetySpec();
    const plan = planSemanticSafetySuite(spec);
    const observations = plan.map(expectedObservation);
    const quoted = observations.find(
      (item) => item.caseId === "v030-quoted-ohm/quoted@identity",
    )!;
    const rename = observations.find(
      (item) => item.caseId === "v030-complete-rename/state-symbol@identity",
    )!;
    const damaged = observations.map((item) => {
      if (item === quoted) return { ...item, decision: "established" as const };
      if (item === rename) {
        return {
          ...item,
          rename: { ...item.rename, edits: item.rename.edits.slice(1) },
        };
      }
      return item;
    });
    const score = scoreSemanticSafetySuite(spec, plan, damaged);
    expect(score.failures.some((failure) => failure.includes("v030-quoted-ohm") && failure.includes("decision established"))).toBe(true);
    expect(score.failures.some((failure) => failure.includes("v030-complete-rename") && failure.includes("rename edits are incomplete"))).toBe(true);
  });

  test("requires an established relation to disappear at lifecycle review", async () => {
    const spec = await loadSemanticSafetySpec();
    const plan = planSemanticSafetySuite(spec);
    const observations = plan.map(expectedObservation);
    const afterId = "v030-period-retraction/after@identity";
    const damaged = observations.map((item) =>
      item.caseId === afterId
        ? {
            ...item,
            relations: [{
              relationId: "period-frequency-reciprocity",
              roles: [
                { role: "frequency", symbol: "f" },
                { role: "period", symbol: "T" },
              ],
              sourceGrounded: true,
            }],
          }
        : item,
    );
    const score = scoreSemanticSafetySuite(spec, plan, damaged);
    expect(
      score.safetyFailures.some((failure) =>
        failure.includes("retained period-frequency-reciprocity after retraction"),
      ),
    ).toBe(true);
  });

  test("permutes document order only for disconnected snapshots", () => {
    expect(
      semanticSafetyTransformApplicable(
        {
          id: "disconnected",
          documents: [
            { content: "$x$", fileId: "a", path: "a.tex" },
            { content: "$y$", fileId: "b", path: "b.tex" },
          ],
        },
        "document-order",
      ),
    ).toBe(true);
    expect(
      semanticSafetyTransformApplicable(
        {
          id: "included",
          documents: [
            { content: "\\input{b}", fileId: "a", path: "a.tex" },
            { content: "$y$", fileId: "b", path: "b.tex" },
          ],
        },
        "document-order",
      ),
    ).toBe(false);
  });

  test("reorders opposed claims without moving their declaration preamble", async () => {
    const plan = planSemanticSafetySuite(await loadSemanticSafetySpec());
    const reordered = plan.find(
      (item) =>
        item.sourceCaseId === "v030-opposed-comparison" &&
        item.transform === "opposition-order",
    )!;
    const content = reordered.documents[0]!.content;
    expect(content.startsWith("Let \\(x\\) denote")).toBe(true);
    expect(content.indexOf("second normative claim")).toBeLessThan(
      content.indexOf("first normative claim"),
    );
  });
});

function expectedObservation(
  item: PlannedSemanticSafetyCase,
): SemanticSafetyObservation {
  const navigation = item.expected.navigation;
  const definitions =
    navigation.mode === "exact"
      ? navigation.definition.map((anchor) =>
          resolveSemanticSafetyAnchor(item.documents, anchor),
        )
      : [];
  const references =
    navigation.mode === "exact"
      ? navigation.references.map((anchor) =>
          resolveSemanticSafetyAnchor(item.documents, anchor),
        )
      : [];
  const renameLocations =
    navigation.mode === "exact"
      ? navigation.rename.map((anchor) =>
          resolveSemanticSafetyAnchor(item.documents, anchor),
        )
      : [];
  const navigationDocument = item.navigationCursor
    ? item.documents.find(
        (document) => document.fileId === item.navigationCursor!.fileId,
      )
    : undefined;
  const navigationOffset = item.navigationCursor
    ? semanticSafetyCursorOffset(item.documents, item.navigationCursor)
    : undefined;
  return {
    caseId: item.id,
    decision: item.expected.decisions[0]!,
    definitions,
    prepareRename:
      navigation.mode === "exact" &&
      item.navigationCursor &&
      navigationDocument &&
      navigationOffset !== undefined
        ? {
            fileId: item.navigationCursor.fileId,
            path: navigationDocument.path,
            placeholder: navigation.placeholder,
            range: {
              startOffset: navigationOffset,
              endOffset: navigationOffset + navigation.expectedText.length,
            },
          }
        : {},
    problemCodes: [],
    meaningRelationId: item.expected.relations[0]?.relationId ?? null,
    proofGrounded: item.expected.proofGrounded,
    references,
    relations: item.expected.relations.map((relation) => ({ ...relation })),
    rename: {
      edits:
        navigation.mode === "exact"
          ? renameLocations.map((location) => ({
              ...location,
              expectedText: navigation.expectedText,
              replacementText: navigation.replacementText,
            }))
          : [],
      ...(navigation.mode === "exact" ? { safety: navigation.safety } : {}),
    },
  };
}
