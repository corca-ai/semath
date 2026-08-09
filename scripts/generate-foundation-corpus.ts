import { mkdir, readFile, writeFile } from "node:fs/promises";
import type { FoundationCase, FoundationCorpus } from "../packages/evaluation/src/index";

interface QuantityKind {
  aliases?: string[];
  defaultUnit?: string;
  dimension: { base: string; denominator: number; numerator: number }[];
  id: string;
  title: string;
}

interface Unit {
  aliases?: string[];
  id: string;
  symbol: string;
}

const root = new URL("../", import.meta.url);
const check = process.argv.includes("--check");
const pack = JSON.parse(
  await readFile(new URL("packs/quantities-units/v1.json", root), "utf8"),
) as { quantityKinds: QuantityKind[]; units: Unit[] };
const units = new Map(pack.units.map((unit) => [`quantities-units:${unit.id}`, unit]));

const cases: FoundationCase[] = pack.quantityKinds.map((kind, index) => {
  const unit = kind.defaultUnit ? units.get(kind.defaultUnit) : undefined;
  const symbol = "Q";
  const description = kind.aliases?.[0] ?? kind.title.toLowerCase();
  const unitPhrase = unit ? ` in ${unit.aliases?.[0] ?? unit.symbol}` : "";
  return declarationCase(
    `quantity-kind-${kind.id}`,
    `Let $${symbol}$ be ${description}${unitPhrase}.`,
    symbol,
    {
      dimension: dimensionDisplay(kind.dimension),
      quantityKindId: `quantities-units:${kind.id}`,
      ...(unit ? { unitId: `quantities-units:${unit.id}` } : {}),
    },
    index,
  );
});

const variants = [
  ["force-alias", "let $F$ denote an applied force measured in newtons.", "F", "force", "newton"],
  ["velocity-alias", "we write $V$ for point velocity in metres per second.", "V", "velocity", "metre-per-second"],
  ["duration-alias", "let $T$ be elapsed time in seconds.", "T", "duration", "second"],
  ["current-alias", "let $I$ be branch current in amperes.", "I", "electric-current", "ampere"],
  ["voltage-alias", "here $U$ denotes potential difference in volts.", "U", "voltage", "volt"],
  ["resistance-alias", "define $R$ as resistor resistance in ohms.", "R", "resistance", "ohm"],
  ["acceleration-alias", "let $A$ represent an acceleration vector in metres per second squared.", "A", "acceleration", "metre-per-second-squared"],
  ["length-alias", "let $L$ denote length measured in metres.", "L", "length", "metre"],
  ["frequency-alias", "let $H$ denote frequency in hertz.", "H", "frequency", "hertz"],
] as const;
for (const [id, content, needle, kindId, unitId] of variants) {
  const kind = pack.quantityKinds.find((candidate) => candidate.id === kindId)!;
  cases.push(
    declarationCase(
      id,
      content,
      needle,
      {
        dimension: dimensionDisplay(kind.dimension),
        quantityKindId: `quantities-units:${kindId}`,
        unitId: `quantities-units:${unitId}`,
      },
      cases.length,
    ),
  );
}

for (const [id, content, needle, kindId] of [
  ["area-paraphrase", "let $S$ be cross-sectional area.", "S", "area"],
  ["temperature-paraphrase", "let $T$ be thermodynamic temperature in kelvin.", "T", "temperature"],
] as const) {
  const kind = pack.quantityKinds.find((candidate) => candidate.id === kindId)!;
  cases.push(
    declarationCase(
      id,
      content,
      needle,
      {
        dimension: dimensionDisplay(kind.dimension),
        quantityKindId: `quantities-units:${kindId}`,
        ...(kind.defaultUnit ? { unitId: kind.defaultUnit } : {}),
      },
      cases.length,
    ),
  );
}

for (const [id, content, needle, code] of [
  ["mass-in-seconds", "Let $m$ be mass in seconds.", "$m$", "quantity-unit-dimension-mismatch"],
  ["duration-in-kilograms", "Let $t$ be duration in kilograms.", "$t$", "quantity-unit-dimension-mismatch"],
  ["velocity-in-newtons", "Let $v$ be velocity in newtons.", "$v$", "quantity-unit-dimension-mismatch"],
  ["force-plus-acceleration", "Let $F$ be force, $m$ mass, and $a$ acceleration. $F=m+a$", "F=m+a", "quantity-addition-dimension-mismatch"],
  ["velocity-from-mass-time", "Let $v$ be velocity, $m$ mass, and $t$ duration. $v=m/t$", "v=m/t", "quantity-assignment-dimension-mismatch"],
  ["force-from-mass-time", "Let $F$ be force, $m$ mass, and $t$ duration. $F=m/t$", "F=m/t", "quantity-assignment-dimension-mismatch"],
] as const) {
  cases.push({
    cursor: { edge: "after", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: "main.md" }],
    expectation: { diagnosticCode: code },
    id,
    variationTags: ["dimension-mismatch", "hard-negative", "semantic-mutation", "unit-conflict", "wrong-role"],
  });
}

for (const [id, content, needle, symbol, dimension] of [
  ["velocity-propagation", "Let $d$ be length and $t$ duration. $v=d/t$. The derived value is $v$.", "v$", "v", "length · time^-1"],
  ["force-propagation", "Let $m$ be mass and $a$ acceleration. $F=m*a$. The derived value is $F$.", "F$", "F", "length · mass · time^-2"],
  ["power-propagation", "Let $F$ be force and $v$ velocity. $P=F\\cdot v$. The derived value is $P$.", "P$", "P", "length^2 · mass · time^-3"],
  ["current-propagation", "Let $q$ be electric charge and $t$ duration. $I=q/t$. The derived value is $I$.", "I$", "I", "electric-current"],
  ["alias-propagation", "Let $v$ be velocity. $u=v$. The derived value is $u$.", "u$", "u", "length · time^-1"],
  ["addition-propagation", "Let $x$ be length. Let $y$ be length. $s=x+y$. The derived value is $s$.", "s$", "s", "length"],
] as const) {
  cases.push({
    cursor: { edge: "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: "main.md" }],
    expectation: { dimension, symbol },
    id,
    variationTags: [
      "dimensions",
      "english-declarations",
      "role-prose",
      "same-dimension",
      "typed",
    ],
  });
}

if (cases.length !== 46) throw new Error(`expected 46 foundation cases, got ${cases.length}`);
const corpus: FoundationCorpus = {
  cases,
  domain: "quantities-foundation",
  schemaVersion: 1,
};
const output = `${JSON.stringify(corpus, null, 2)}\n`;
const path = new URL("fixtures/foundation/quantities-units.json", root);
if (check) {
  if (await readFile(path, "utf8").catch(() => "") !== output) {
    throw new Error("fixtures/foundation/quantities-units.json: generated corpus is stale");
  }
} else {
  await mkdir(new URL("fixtures/foundation/", root), { recursive: true });
  await writeFile(path, output);
}
console.log(`${check ? "verified" : "generated"} quantities foundation corpus (${cases.length} cases)`);

function declarationCase(
  id: string,
  content: string,
  needle: string,
  expectation: FoundationCase["expectation"],
  index: number,
): FoundationCase {
  return {
    cursor: { edge: index % 2 ? "after" : "before", fileId: "main", needle },
    documents: [{ content, fileId: "main", path: index % 3 ? "main.md" : "main.tex" }],
    expectation,
    id,
    variationTags: [
      "conventional-notation",
      "dimensions",
      "english-declarations",
      "role-prose",
      ...(expectation.unitId ? ["unit-context"] : []),
    ],
  };
}

function dimensionDisplay(
  dimension: readonly { base: string; denominator: number; numerator: number }[],
): string {
  if (!dimension.length) return "dimensionless";
  return [...dimension]
    .sort((left, right) => left.base.localeCompare(right.base))
    .map(({ base, denominator, numerator }) => {
      if (numerator === 1 && denominator === 1) return base;
      if (denominator === 1) return `${base}^${numerator}`;
      return `${base}^(${numerator}/${denominator})`;
    })
    .join(" · ");
}
