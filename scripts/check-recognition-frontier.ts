import { readFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  classifyRecognitionFrontier,
  frontierSignals,
  parseRecognitionFrontier,
  scoreRecognitionFrontier,
  type RecognitionFrontierObservation,
} from "../packages/evaluation/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const frontier = parseRecognitionFrontier(
  JSON.parse(
    await readFile(
      new URL(
        "../fixtures/challenge/recognition-frontier-v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);
const results = runSemanticEvaluation(
  frontier.cases,
  "recognition-frontier-v1",
);
const observations = frontier.cases.map(
  (item, index): RecognitionFrontierObservation => {
    const result = results[index];
    if (!result || result.value.kind !== "semanticView") {
      throw new Error(`${item.id}: semanticView result is unavailable`);
    }
    const view = result.value.view;
    const signals = frontierSignals(view, hasCursorSyntax(item));
    return {
      caseId: item.id,
      decision: view.decision.status,
      relationId:
        view.decision.status === "established" ||
        view.decision.status === "partial"
          ? view.decision.meaning.relationId
          : null,
      signals,
      stage: classifyRecognitionFrontier(signals),
    };
  },
);
const score = scoreRecognitionFrontier(frontier, observations);
const baselineMoved = frontier.cases.filter((item, index) => {
  const observed = observations[index];
  return (
    observed?.decision !== item.baseline.decision ||
    observed.stage !== item.baseline.stage
  );
});

console.log(
  `recognition frontier: ${score.passed}/${score.cases}; risk ${score.risk.total}; baseline transitions ${baselineMoved.length}`,
);
if (process.env.SEMATH_FRONTIER_REPORT) {
  await Bun.write(
    process.env.SEMATH_FRONTIER_REPORT,
    `${JSON.stringify(
      {
        ...score,
        baselineTransitions: baselineMoved.map((item) => item.id),
        observations,
      },
      null,
      2,
    )}\n`,
  );
}
if (
  score.failures.length &&
  process.env.SEMATH_FRONTIER_ALLOW_FAILURES !== "1"
) {
  throw new Error(
    `recognition frontier failed:\n${score.failures.join("\n")}`,
  );
}

function hasCursorSyntax(
  item: (typeof frontier.cases)[number],
): boolean {
  const document = item.documents.find(
    (document) => document.fileId === item.cursor.fileId,
  );
  if (!document) return false;
  const offset = document.content.indexOf(item.cursor.needle);
  if (offset < 0 || offset !== document.content.lastIndexOf(item.cursor.needle)) {
    return false;
  }
  const syntax = new LatexSyntaxService();
  syntax.reset({ documents: [{ ...document, documentVersion: 1 }] });
  const snapshot = syntax.getFile(document.fileId);
  if (!snapshot) return false;
  const adapted = adaptWasmtexDocument({
    content: document.content,
    language: /\.md$/iu.test(document.path) ? "markdown" : "latex",
    syntax: snapshot,
  });
  return adapted.mathRoots.some(
    (root) =>
      root.contentRange.startOffset <= offset &&
      offset < root.contentRange.endOffset,
  );
}
