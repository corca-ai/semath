import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { assertSameCanonicalPath } from "./canonical-path";

describe("canonical path identity", () => {
  test("accepts aliases and rejects a different canonical target", async () => {
    const root = await mkdtemp(join(tmpdir(), "semath-canonical-path-"));
    const target = join(root, "target");
    const other = join(root, "other");
    const alias = join(root, "alias");
    try {
      await mkdir(target);
      await mkdir(other);
      await symlink(target, alias);
      await expect(
        assertSameCanonicalPath(alias, target, "work root"),
      ).resolves.toBeUndefined();
      await expect(
        assertSameCanonicalPath(alias, other, "work root"),
      ).rejects.toThrow("canonical path does not match");
    } finally {
      await rm(root, { force: true, recursive: true });
    }
  });
});
