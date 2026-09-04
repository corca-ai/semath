import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";

const workflow = await readFile(
  new URL("../.github/retired-workflows/fresh-blind-release.yml", import.meta.url),
  "utf8",
);

describe("fresh blind workflow integrity", () => {
  test("serializes every release in one non-cancelling concurrency group", () => {
    expect(workflow).toContain("group: semath-fresh-blind-one-shot");
    expect(workflow).toContain("cancel-in-progress: false");
  });

  test("proves the reservation with credentials before token-free execution", () => {
    expect(workflow.match(/GITHUB_TOKEN:/gu)).toHaveLength(3);
    const reservation = workflow.indexOf(
      "- name: Permanently reserve the release identity and fixture seal",
    );
    const proof = workflow.indexOf(
      "- name: Prove the permanent reservation before execution",
    );
    const execute = workflow.indexOf(
      "- name: Execute the reserved fresh fixture exactly once",
    );
    expect(reservation).toBeGreaterThan(-1);
    expect(proof).toBeGreaterThan(reservation);
    expect(execute).toBeGreaterThan(proof);
    expect(workflow.slice(proof, execute)).toContain("GITHUB_TOKEN");

    const execution = workflow.slice(
      execute,
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

  test("installs the pinned documentation linter before preflight", () => {
    const install = workflow.indexOf("- name: Install awiki 0.5.0");
    const preflight = workflow.indexOf(
      "- name: Run all pre-blind gates without release credentials",
    );

    expect(install).toBeGreaterThan(-1);
    expect(install).toBeLessThan(preflight);
    expect(workflow).toContain(
      "awiki/releases/download/v0.5.0/awiki-x86_64-unknown-linux-gnu.tar.xz",
    );
    expect(workflow).toContain(
      "c0b7ee22c089130c5ace0cd7201cf8c39a48afbfbc220463b03f5ba41fe8200e",
    );
    expect(workflow).toContain('echo "$HOME/.local/bin" >> "$GITHUB_PATH"');
  });
});
