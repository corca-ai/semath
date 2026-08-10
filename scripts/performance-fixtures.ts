import type { LatexDocumentInput } from "wasmtex/syntax";

export const PERFORMANCE_FIXTURE_FAMILIES = [
  "reported-ece",
  "decorated-and-styled",
  "dense-matrix",
  "unicode-and-combining",
  "malformed-recovery",
  "binder-and-rename",
  "document-shaped-report",
  "scoped-neighbor",
] as const;

export type PerformanceFixtureFamily = (typeof PERFORMANCE_FIXTURE_FAMILIES)[number];

export interface PerformanceFixtureDocument extends LatexDocumentInput {
  family: PerformanceFixtureFamily;
  queryOffset: number;
}

export function buildPerformanceDocuments(count: number): readonly PerformanceFixtureDocument[] {
  return Array.from({ length: count }, (_, index) => performanceDocument(index));
}

export function editPerformanceDocument(
  source: PerformanceFixtureDocument,
  run: number,
): PerformanceFixtureDocument {
  const content = `${source.content}\n% measured leaf edit ${run}`;
  return {
    ...source,
    content,
    documentVersion: source.documentVersion + 1,
    queryOffset: source.queryOffset,
  };
}

export function semanticallyEditPerformanceDocument(
  source: PerformanceFixtureDocument,
): PerformanceFixtureDocument {
  const marker = "\\operatorname{ECE}";
  const content = source.content.includes(marker)
    ? source.content.replace(marker, `${marker}_{updated}`)
    : `${source.content}\n$z_{updated}=1$`;
  return {
    ...source,
    content,
    documentVersion: source.documentVersion + 1,
    queryOffset: source.queryOffset,
  };
}

function performanceDocument(index: number): PerformanceFixtureDocument {
  const family = PERFORMANCE_FIXTURE_FAMILIES[index % PERFORMANCE_FIXTURE_FAMILIES.length]!;
  const symbol = `p${index}`;
  const common = `Let ${symbol} denote the probability assigned to event A${index}.`;
  const body = fixtureBody(family, index, symbol);
  const content = `${common}\n${body}`;
  return {
    content,
    documentVersion: 1,
    family,
    fileId: `section-${index}`,
    language: "latex",
    path: `section-${index}.tex`,
    queryOffset: content.indexOf(symbol, common.length),
  };
}

function fixtureBody(family: PerformanceFixtureFamily, index: number, symbol: string): string {
  switch (family) {
    case "reported-ece":
      return [
        "Expected calibration error (ECE) uses confidence bins $B_m$.",
        `\$${symbol}=\\operatorname{ECE}=\\sum_{m=1}^{M}\\frac{|B_m|}{n}\\left|\\operatorname{acc}(B_m)-\\operatorname{conf}(B_m)\\right|\$`,
      ].join("\n");
    case "decorated-and-styled":
      return `\$${symbol}=\\hat{\\mathbf y}_{t+1}+\\widetilde{\\mathcal L}(\\symbf{x})\$`;
    case "dense-matrix":
      return `\$${symbol}=\\begin{bmatrix}a_{11}&a_{12}&a_{13}\\\\a_{21}&a_{22}&a_{23}\\\\a_{31}&a_{32}&a_{33}\\end{bmatrix}x\$`;
    case "unicode-and-combining":
      return `\$${symbol}=x̂+𝛼_${index}+\\mathrm{ECE}+\\operatorname{ECE}\$`;
    case "malformed-recovery":
      return `\$${symbol}=\\frac{\\hat{x_${index}}{\\left(y+z\\right.\$`;
    case "binder-and-rename":
      return `\$${symbol}=\\sum_{k=1}^{n} a_k+\\int_0^1 f(t)\\,dt\$`;
    case "document-shaped-report":
      return [
        "\\section{Background}",
        "The surrounding experiment reports several independent measurements before the calibrated result.",
        "$\\xi_{\\mathrm{aux}}=17$",
        "\\section{Reported result}",
        `\$${symbol}=\\operatorname{ECE}=\\sum_{m=1}^{M}\\frac{|B_m|}{n}\\left|\\operatorname{acc}(B_m)-\\operatorname{conf}(B_m)\\right|\$`,
        "$\\zeta_{\\mathrm{aux}}=19$",
      ].join("\n");
    case "scoped-neighbor":
      return [
        "\\section{Independent notation}",
        "$\\mathbf{q}_{\\mathrm{aux}}=\\frac{1}{2}$",
        "\\section{Current result}",
        `\$${symbol}=\\widehat{y}_{t+1}+\\operatorname{loss}(x)\$`,
        "$\\frac{1}{$",
      ].join("\n");
  }
}
