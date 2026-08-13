import { describe, expect, test } from "bun:test";
import { completeLifecycleUpsertIds } from "./fresh-blind-lifecycle-plan";

describe("fresh blind lifecycle planning", () => {
  test("always forwards a directly changed document even when syntax is reusable", () => {
    expect(completeLifecycleUpsertIds(new Set(["main"]), [])).toEqual(["main"]);
  });

  test("adds dependents once in deterministic order", () => {
    expect(
      completeLifecycleUpsertIds(
        new Set(["main", "shared"]),
        ["dependent", "main"],
      ),
    ).toEqual(["dependent", "main", "shared"]);
  });
});
