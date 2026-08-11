import { readFile } from "node:fs/promises";
import { parseDomainRoutingChallenge } from "../packages/evaluation/src/index";

const fixture = JSON.parse(await readFile(new URL("../fixtures/challenge/domain-routing-v1.json", import.meta.url), "utf8"));
const challenge = parseDomainRoutingChallenge(fixture);
const normalized = challenge.cases.map((item) => item.documents.map((document) => document.content.toLowerCase().replaceAll(/\s+/gu, " ").trim()).join("\n"));
if (new Set(normalized).size !== normalized.length) throw new Error("domain challenge contains duplicate normalized documents");
const challengePath = "fixtures/challenge/domain-routing-v1.json";
const otherDocuments = new Set<string>();
for await (const path of new Bun.Glob("fixtures/**/*.json").scan(".")) {
  if (path === challengePath) continue;
  const value: unknown = JSON.parse(await readFile(path, "utf8"));
  collectDocumentContent(value, otherDocuments);
}
const leaked = normalized.filter((document) => otherDocuments.has(document));
if (leaked.length) throw new Error(`domain challenge leaked into ${leaked.length} development or generated fixture(s)`);
console.log(`domain routing fixture: ${challenge.cases.length} independent cases, ${challenge.reviewedCollisionComponents.length} reviewed collision components`);

function collectDocumentContent(value: unknown, output: Set<string>): void {
  if (Array.isArray(value)) {
    for (const item of value) collectDocumentContent(item, output);
    return;
  }
  if (typeof value !== "object" || value === null) return;
  const record = value as Record<string, unknown>;
  if (typeof record.content === "string") {
    output.add(record.content.toLowerCase().replaceAll(/\s+/gu, " ").trim());
  }
  for (const item of Object.values(record)) collectDocumentContent(item, output);
}
