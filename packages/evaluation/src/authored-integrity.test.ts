import { describe, expect, test } from "bun:test";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  authoredMathFingerprints,
  authoredProseShingles,
  compareAuthoredIntegrityProfiles,
} from "./authored-integrity";

describe("authored corpus integrity", () => {
  test("recognizes alpha-renamed formulas through the one wasmtex CST", () => {
    const first = parse("First derivation: $a+b=c$.");
    const second = parse("Independent notation: $x+y=z$.");
    expect(authoredMathFingerprints(first.syntax)).toEqual(
      authoredMathFingerprints(second.syntax),
    );
  });

  test("preserves repeated-symbol structure while anonymizing spelling", () => {
    const repeated = parse("$x+x=y$");
    const distinct = parse("$a+b=c$");
    expect(authoredMathFingerprints(repeated.syntax)).not.toEqual(
      authoredMathFingerprints(distinct.syntax),
    );
  });

  test("compares only visible prose after math is removed", () => {
    const left = parse("The reviewed relation applies to the calibrated sample $a=b$ only.");
    const right = parse("The reviewed relation applies to the calibrated sample $x=y$ only.");
    const leftProfile = {
      id: "development",
      mathFingerprints: authoredMathFingerprints(left.syntax),
      proseShingles: authoredProseShingles(left.content, left.syntax),
    };
    const rightProfile = {
      id: "holdout",
      mathFingerprints: authoredMathFingerprints(right.syntax),
      proseShingles: authoredProseShingles(right.content, right.syntax),
    };
    expect(compareAuthoredIntegrityProfiles([leftProfile], [rightProfile])).toEqual([
      {
        developmentId: "development",
        exactMath: true,
        holdoutId: "holdout",
        mathSimilarity: 1,
        proseSimilarity: 1,
      },
    ]);
  });
});

function parse(content: string) {
  const service = new LatexSyntaxService();
  service.reset({
    documents: [
      {
        content,
        documentVersion: 1,
        fileId: "test.md",
        language: "markdown",
        path: "test.md",
      },
    ],
  });
  const syntax = service.getFile("test.md");
  if (!syntax) throw new Error("missing syntax fixture");
  return { content, syntax };
}
