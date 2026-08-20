import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";

const workflow = await readFile(
  new URL("../.github/workflows/fresh-blind-release.yml", import.meta.url),
  "utf8",
);

describe("fresh blind workflow integrity", () => {
  test("serializes every release in one non-cancelling concurrency group", () => {
    expect(workflow).toContain("group: semath-fresh-blind-one-shot");
    expect(workflow).toContain("cancel-in-progress: false");
  });

  test("keeps the release token out of preflight, execution, and terminalization", () => {
    expect(workflow.match(/GITHUB_TOKEN:/gu)).toHaveLength(2);
    const execution = workflow.slice(
      workflow.indexOf(
        "- name: Execute the reserved fresh fixture exactly once",
      ),
      workflow.indexOf(
        "- name: Terminalize every normally returned reserved execution",
      ),
    );
    const terminalization = workflow.slice(
      workflow.indexOf(
        "- name: Terminalize every normally returned reserved execution",
      ),
      workflow.indexOf("- name: Upload immutable release evidence"),
    );
    expect(execution).not.toContain("GITHUB_TOKEN");
    expect(terminalization).not.toContain("GITHUB_TOKEN");
  });

  test("attests any uploaded reserved terminal outcome, including failures", () => {
    expect(workflow).toContain(
      "always() && needs.evaluate.outputs.reserved == 'success' && needs.evaluate.outputs.artifact-id != ''",
    );
    expect(workflow).toContain(".attested/receipt.json");
    expect(workflow).toContain(".attested/semath_wasm_bg.wasm");
    expect(workflow).toContain(".attested/semath-0.18.0.tgz");
  });

  test("pins every third-party action to a full commit", () => {
    const uses = [...workflow.matchAll(/^\s*- uses: ([^\s]+)$/gmu)].map(
      (match) => match[1]!,
    );
    expect(uses.length).toBeGreaterThan(0);
    expect(uses.every((use) => /@[0-9a-f]{40}$/u.test(use))).toBe(true);
  });
});
