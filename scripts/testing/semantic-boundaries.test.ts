import { describe, expect, test } from "bun:test";
import {
  FORBIDDEN_SEMANTIC_PATHS,
  checkSemanticBoundaries,
} from "./semantic-boundaries";

describe("semantic architecture boundaries", () => {
  test("finds only paths whose complete forbidden signature occurs", () => {
    const rule = FORBIDDEN_SEMANTIC_PATHS[0];
    const complete = Object.fromEntries([
      [rule.path, rule.markers.join("\n")],
    ]);
    expect(checkSemanticBoundaries(complete)).toEqual([
      { id: rule.id, path: rule.path },
    ]);
    expect(checkSemanticBoundaries({ [rule.path]: rule.markers[0] })).toEqual([]);
  });

  test("returns findings in the deterministic rule order", () => {
    const sources: Record<string, string> = {};
    for (const rule of FORBIDDEN_SEMANTIC_PATHS) {
      const signature =
        "scopeStart" in rule
          ? `${rule.scopeStart}\n${rule.markers.join("\n")}${rule.scopeEnd}`
          : rule.markers.join("\n");
      sources[rule.path] = `${sources[rule.path] ?? ""}\n${signature}`;
    }
    expect(checkSemanticBoundaries(sources).map((finding) => finding.id)).toEqual(
      FORBIDDEN_SEMANTIC_PATHS.map((rule) => rule.id),
    );
  });
});
