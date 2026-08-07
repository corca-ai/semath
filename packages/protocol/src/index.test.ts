import { describe, expect, test } from "bun:test";
import { SEMATH_PROTOCOL_VERSION, type ProjectSnapshot } from "./index";

describe("protocol", () => {
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
});

