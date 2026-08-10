import { describe, expect, test } from "bun:test";
import {
  LEGACY_SEMANTIC_PATHS,
  auditLegacySemanticPaths,
} from "./legacy-path-audit";

describe("legacy semantic path audit", () => {
  test("finds only paths whose complete legacy signature remains", () => {
    const rule = LEGACY_SEMANTIC_PATHS[0];
    const complete = Object.fromEntries([
      [rule.path, rule.markers.join("\n")],
    ]);
    expect(auditLegacySemanticPaths(complete)).toEqual([
      { id: rule.id, ownerIssue: rule.ownerIssue, path: rule.path },
    ]);
    expect(auditLegacySemanticPaths({ [rule.path]: rule.markers[0] })).toEqual([]);
  });

  test("returns findings in the reviewed ownership order", () => {
    const sources: Record<string, string> = {};
    for (const rule of LEGACY_SEMANTIC_PATHS) {
      const signature =
        "scopeStart" in rule
          ? `${rule.scopeStart}\n${rule.markers.join("\n")}${rule.scopeEnd}`
          : rule.markers.join("\n");
      sources[rule.path] = `${sources[rule.path] ?? ""}\n${signature}`;
    }
    expect(auditLegacySemanticPaths(sources).map((finding) => finding.id)).toEqual(
      LEGACY_SEMANTIC_PATHS.map((rule) => rule.id),
    );
  });
});
