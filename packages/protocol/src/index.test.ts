import { describe, expect, test } from "bun:test";
import {
  SEMATH_PROTOCOL_VERSION,
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
    expect(snapshot.protocolVersion).toBe(14);
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
});
