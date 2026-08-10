import { join } from "node:path";
import {
  LEGACY_SEMANTIC_PATHS,
  auditLegacySemanticPaths,
} from "../packages/evaluation/src/legacy-path-audit";

const root = join(import.meta.dir, "..");
const sources = Object.fromEntries(
  await Promise.all(
    [...new Set(LEGACY_SEMANTIC_PATHS.map((rule) => rule.path))].map(
      async (path) => [path, await Bun.file(join(root, path)).text()] as const,
    ),
  ),
);
const findings = auditLegacySemanticPaths(sources);
console.log(JSON.stringify({ findings, schemaVersion: 1 }, null, 2));
if (process.argv.includes("--check") && findings.length) {
  throw new Error(
    `legacy semantic paths remain:\n${findings
      .map((finding) => `${finding.id} (${finding.path}, #${finding.ownerIssue})`)
      .join("\n")}`,
  );
}
