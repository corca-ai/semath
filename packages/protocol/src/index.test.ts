import { describe, expect, test } from "bun:test";
import {
  SEMATH_PROTOCOL_VERSION,
  type ConventionalCandidateInfo,
  type LawConditionInfo,
  type ProjectSnapshot,
  type SymbolInfo,
} from "./index";

describe("protocol", () => {
  test("publishes the hard-cutover protocol version", () => {
    const snapshot: ProjectSnapshot = {
      documents: [],
      epoch: "project:1",
      inventoryVersion: 1,
      projectId: "project",
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    };
    expect(snapshot.protocolVersion).toBe(16);
  });

  test("allows omitted empty role collections from the wire format", () => {
    const info: SymbolInfo = {
      definitions: [],
      diagnostics: [],
      location: {
        fileId: "main",
        path: "main.md",
        range: { endOffset: 2, startOffset: 1 },
      },
      occurrenceId: {
        documentVersion: 1,
        fileId: "main",
        localId: 0,
      },
      notation: [],
      sourceNotation: "x",
      shapes: [],
      symbol: "x",
      truncated: false,
    };
    expect(info.roles).toBeUndefined();
  });

  test("keeps operator requirements separate from display labels", () => {
    const condition: LawConditionInfo = {
      conditionId: "linear-map",
      evidence: [],
      kind: "operator-property",
      label: "The operator is linear.",
      operatorProperty: "linear",
      status: "required",
      subjects: ["A"],
    };
    expect(condition.operatorProperty).toBe("linear");
  });

  test("keeps conventional candidates structurally non-authoritative", () => {
    const candidate: ConventionalCandidateInfo = {
      bindings: [],
      candidateId: "conventional/linear-algebra/matrix-vector-product/1:5",
      disposition: "conventional-candidate",
      evidence: [],
      lawId: "matrix-vector-product",
      packId: "linear-algebra",
      packVersion: "1.4.0",
      relation: {
        conditions: [],
        description: "A matrix maps a vector.",
        evidence: [],
        range: { endOffset: 5, startOffset: 1 },
        relationId: "linear-algebra:matrix-vector-product",
        roles: [],
        title: "Matrix-vector product",
      },
      relevance: { evidence: [], support: "supported" },
      requirements: [],
      title: "Matrix-vector product",
    };
    expect(candidate.disposition).toBe("conventional-candidate");
  });
});
