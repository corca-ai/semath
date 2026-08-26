import { readFile, writeFile } from "node:fs/promises";
import {
  parseSemanticContinuityFixture,
  scoreSemanticContinuity,
  type SemanticContinuityObservation,
} from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const fixture = parseSemanticContinuityFixture(
  JSON.parse(
    await readFile(
      new URL("../fixtures/challenge/semantic-continuity-v1.json", import.meta.url),
      "utf8",
    ),
  ),
);
const results = runSemanticEvaluation(fixture.cases, "semantic-continuity-v1");
const observations: SemanticContinuityObservation[] = fixture.cases.map(
  (item, index) => {
    const value = results[index]?.value;
    if (value?.kind !== "semanticView") {
      throw new Error(`${item.id}: expected semanticView result`);
    }
    const view = value.view;
    const relationIds = new Set(
      view.context.relations.map((relation) => relation.relationId),
    );
    if (
      (view.decision.status === "established" ||
        view.decision.status === "partial") &&
      view.decision.meaning.relationId
    ) {
      relationIds.add(view.decision.meaning.relationId);
    }
    return {
      caseId: item.id,
      decision: view.decision.status,
      definitions: (view.symbol?.definitions ?? []).map(
        (definition) => definition.description,
      ),
      formulaDecision: view.authoringContext?.disposition ?? null,
      problems: view.diagnostics.length,
      relationIds: [...relationIds].sort(),
      shapeKinds: [...new Set((view.symbol?.shapes ?? []).map((shape) => shape.kind))].sort(),
      symbol: view.symbol?.symbol ?? null,
    };
  },
);
const score = scoreSemanticContinuity(fixture, observations);
console.log(
  `semantic continuity: ${score.passed}/${score.cases}; risk ${score.risk.total} ` +
    `(false-establishment ${score.risk.falseEstablishment}, false-conflict ${score.risk.falseConflict}, ` +
    `identity ${score.risk.navigationOrIdentity}, missed ${score.risk.missedCoverage})`,
);
for (const [family, result] of Object.entries(score.families)) {
  console.log(`  ${family}: ${result.passed}/${result.total}`);
}
if (process.env.SEMATH_CONTINUITY_REPORT) {
  await writeFile(
    process.env.SEMATH_CONTINUITY_REPORT,
    `${JSON.stringify({ observations, score }, null, 2)}\n`,
  );
}
if (score.failures.length && process.env.SEMATH_CONTINUITY_ALLOW_FAILURES !== "1") {
  throw new Error(`semantic continuity failed:\n${score.failures.join("\n")}`);
}
