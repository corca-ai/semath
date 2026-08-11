import type {
  LatexDocumentSyntaxSnapshot,
  LatexNotationNode,
} from "wasmtex/syntax";

const GREEK_SYMBOL_COMMANDS = new Set([
  "alpha",
  "beta",
  "chi",
  "delta",
  "Delta",
  "epsilon",
  "varepsilon",
  "eta",
  "gamma",
  "Gamma",
  "iota",
  "kappa",
  "lambda",
  "Lambda",
  "mu",
  "nu",
  "omega",
  "Omega",
  "omicron",
  "phi",
  "varphi",
  "Phi",
  "pi",
  "varpi",
  "Pi",
  "psi",
  "Psi",
  "rho",
  "varrho",
  "sigma",
  "Sigma",
  "tau",
  "theta",
  "vartheta",
  "Theta",
  "upsilon",
  "Upsilon",
  "xi",
  "Xi",
  "zeta",
]);

export interface AuthoredIntegrityProfile {
  readonly id: string;
  readonly mathFingerprints: readonly string[];
  readonly proseShingles: readonly string[];
}

export interface AuthoredIntegrityComparison {
  readonly developmentId: string;
  readonly exactMath: boolean;
  readonly holdoutId: string;
  readonly mathSimilarity: number;
  readonly proseSimilarity: number;
}

/**
 * Produce alpha-renaming-insensitive fingerprints from wasmtex's neutral CST.
 * This is corpus evidence tooling, not a second TeX parser or semantic path.
 */
export function authoredMathFingerprints(
  syntax: LatexDocumentSyntaxSnapshot,
): readonly string[] {
  return syntax.mathRoots.map((root) => {
    const symbols = new Map<string, string>();
    const symbol = (surface: string): string => {
      const known = symbols.get(surface);
      if (known) return known;
      const value = `identifier-${symbols.size}`;
      symbols.set(surface, value);
      return value;
    };
    const visit = (nodeId: number): unknown => {
      const node = syntax.nodes[nodeId];
      if (!node) throw new Error(`missing wasmtex notation node ${nodeId}`);
      const argumentRoles = new Map(
        (node.arguments ?? []).map((argument) => [
          argument.node,
          `${argument.role}:${argument.syntax}`,
        ]),
      );
      return {
        children: node.children.map((child) => [
          argumentRoles.get(child) ?? null,
          visit(child),
        ]),
        kind: node.kind,
        lexicalClass: node.lexicalClass ?? null,
        mathClass: node.mathClass ?? null,
        name: anonymizedName(node, symbol),
        state: node.state,
        text: anonymizedText(node, symbol),
      };
    };
    return JSON.stringify(visit(root.node));
  });
}

/** Visible-prose shingles use wasmtex spans, so math and TeX controls never leak in. */
export function authoredProseShingles(
  content: string,
  syntax: LatexDocumentSyntaxSnapshot,
  width = 5,
): readonly string[] {
  if (!Number.isSafeInteger(width) || width < 2 || width > 12) {
    throw new Error("prose shingle width must be an integer from 2 through 12");
  }
  const visible = syntax.visibleProse
    .map(({ range }) => content.slice(range.startOffset, range.endOffset))
    .join(" ")
    .normalize("NFKC")
    .toLocaleLowerCase("en-US");
  const words = visible.match(/[\p{L}\p{N}]+(?:['’-][\p{L}\p{N}]+)*/gu) ?? [];
  const output = new Set<string>();
  for (let index = 0; index + width <= words.length; index++) {
    output.add(words.slice(index, index + width).join(" "));
  }
  return [...output].sort();
}

export function compareAuthoredIntegrityProfiles(
  development: readonly AuthoredIntegrityProfile[],
  holdout: readonly AuthoredIntegrityProfile[],
): readonly AuthoredIntegrityComparison[] {
  return development.flatMap((left) =>
    holdout.map((right) => ({
      developmentId: left.id,
      exactMath:
        left.mathFingerprints.length > 0 &&
        setsEqual(left.mathFingerprints, right.mathFingerprints),
      holdoutId: right.id,
      mathSimilarity: jaccard(left.mathFingerprints, right.mathFingerprints),
      proseSimilarity: jaccard(left.proseShingles, right.proseShingles),
    })),
  );
}

function anonymizedName(
  node: LatexNotationNode,
  symbol: (surface: string) => string,
): string | null {
  if (
    node.kind === "command" &&
    node.name !== undefined &&
    GREEK_SYMBOL_COMMANDS.has(node.name)
  ) {
    return symbol(`\\${node.name}`);
  }
  return node.name ?? null;
}

function anonymizedText(
  node: LatexNotationNode,
  symbol: (surface: string) => string,
): string | null {
  if (node.lexicalClass === "identifier" && node.text !== undefined) {
    return symbol(node.text);
  }
  return node.text?.replaceAll(/\s+/gu, " ").trim() ?? null;
}

function jaccard(left: readonly string[], right: readonly string[]): number {
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  const union = new Set([...leftSet, ...rightSet]);
  if (union.size === 0) return 0;
  let intersection = 0;
  for (const value of leftSet) {
    if (rightSet.has(value)) intersection++;
  }
  return intersection / union.size;
}

function setsEqual(left: readonly string[], right: readonly string[]): boolean {
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  return (
    leftSet.size === rightSet.size &&
    [...leftSet].every((value) => rightSet.has(value))
  );
}
