import { describe, expect, test } from "bun:test";
import {
  type FormulaRecognition,
  SEMATH_PROTOCOL_VERSION,
  type ProjectSnapshot,
  type SymbolInfo,
} from "./index";

const v010Recognition: FormulaRecognition = {
  bindings: [],
  evidence: [],
  packId: "linear-algebra",
  packVersion: "1.0.0",
  patternId: "matrix-vector-product",
  range: { endOffset: 2, startOffset: 0 },
  rank: 100,
  result: { kind: "vector" },
  title: "Matrix-vector product",
};

describe("protocol", () => {
  test("keeps additive recognition metadata compatible with protocol v1", () => {
    expect(v010Recognition.description).toBeUndefined();
  });
  test("keeps the public version explicit", () => {
    const snapshot: ProjectSnapshot = {
      documents: [],
      epoch: "project:1",
      inventoryVersion: 1,
      projectId: "project",
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    };
    expect(snapshot.protocolVersion).toBe(1);
  });

  test("allows omitted empty role collections from the wire format", () => {
    const info: SymbolInfo = {
      definitions: [],
      diagnostics: [],
      formulas: [],
      location: {
        fileId: "main",
        path: "main.md",
        range: { endOffset: 2, startOffset: 1 },
      },
      shapes: [],
      symbol: "x",
      truncated: false,
    };
    expect(info.roles).toBeUndefined();
  });
});
