import { readFile } from "node:fs/promises";
import { parseSemanticContinuityFixture } from "../packages/evaluation/src/index";

const path = new URL(
  "../fixtures/challenge/semantic-continuity-v1.json",
  import.meta.url,
);
const fixture = parseSemanticContinuityFixture(
  JSON.parse(await readFile(path, "utf8")),
);
const fixturePath = "fixtures/challenge/semantic-continuity-v1.json";
const normalizedDocuments = fixture.cases.map((item) =>
  item.documents
    .map((document) => normalize(document.content))
    .join("\n"),
);
const otherDocuments = new Set<string>();
for await (const candidate of new Bun.Glob("fixtures/**/*.json").scan(".")) {
  if (candidate === fixturePath) continue;
  collectDocumentContent(
    JSON.parse(await readFile(candidate, "utf8")),
    otherDocuments,
  );
}
const leaked = normalizedDocuments.filter((document) =>
  otherDocuments.has(document),
);
if (leaked.length) {
  throw new Error(
    `semantic continuity holdout leaked into ${leaked.length} development or generated fixture(s)`,
  );
}
const tags = new Set(fixture.cases.flatMap((item) => item.variationTags));
const targetTransitions = fixture.cases.filter(
  (item) =>
    item.baseline.decision !== item.target.decision ||
    item.baseline.problems < item.target.minimumProblems ||
    item.baseline.problems > item.target.maximumProblems,
).length;
if (tags.size < 24) {
  throw new Error("semantic continuity fixture requires at least 24 variation tags");
}
if (targetTransitions < 12) {
  throw new Error("semantic continuity fixture requires at least 12 reviewed target transitions");
}
console.log(
  `semantic continuity fixture: ${fixture.cases.length} cases, ${tags.size} tags, ${targetTransitions} transitions`,
);

function collectDocumentContent(value: unknown, output: Set<string>): void {
  if (Array.isArray(value)) {
    for (const item of value) collectDocumentContent(item, output);
    return;
  }
  if (typeof value !== "object" || value === null) return;
  const item = value as Record<string, unknown>;
  if (typeof item.content === "string") output.add(normalize(item.content));
  for (const child of Object.values(item)) collectDocumentContent(child, output);
}

function normalize(value: string): string {
  return value.toLowerCase().replaceAll(/\s+/gu, " ").trim();
}
