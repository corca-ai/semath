import { describe, expect, test } from "bun:test";
import { createProjectSnapshot } from "./index";

describe("wasmtex adapter", () => {
  test("keeps wasmtex UTF-16 ranges without translating them", () => {
    const region = {
      closed: true,
      contentRange: { endOffset: 9, startOffset: 6 },
      delimiter: "$",
      fullRange: { endOffset: 10, startOffset: 5 },
    };
    const snapshot = createProjectSnapshot({
      documents: [
        {
          content: "😀 한 $x_i$",
          language: "markdown",
          syntax: {
            documentVersion: 2,
            fileId: "f1",
            mathRegions: [region],
            path: "main.md",
            schemaVersion: 1,
          },
        },
      ],
      epoch: "p:1",
      inventoryVersion: 4,
      projectId: "p",
    });
    expect(snapshot.documents[0]?.mathRegions?.[0]).toEqual(region);
  });
});
