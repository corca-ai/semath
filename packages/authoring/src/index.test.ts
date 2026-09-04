import { describe, expect, test } from "bun:test";
import { findForbiddenRuntimeBranches } from "./index";

describe("pack runtime boundaries", () => {
  test("finds pack-specific runtime decisions but ignores tests and data mentions", () => {
    expect(findForbiddenRuntimeBranches([
      { path: "src/infer.rs", source: 'if pack_id == "sample-field" { specialize(); }' },
      { path: "src/catalog.rs", source: 'const ID: &str = "sample-field";' },
      { path: "src/infer.test.ts", source: 'if (packId === "sample-field") fail();' },
      { path: "src/engine_tests.rs", source: 'if pack_id == "sample-field" { fail(); }' },
      { path: "tests/integration.rs", source: 'if pack_id == "sample-field" { fail(); }' },
      { path: "src\\resolver_test.rs", source: 'if pack_id == "sample-field" { fail(); }' },
    ], ["sample-field"])).toEqual([
      {
        id: "sample-field",
        line: 1,
        path: "src/infer.rs",
        sourceLine: 'if pack_id == "sample-field" { specialize(); }',
      },
    ]);
  });

});
