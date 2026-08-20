import { describe, expect, test } from "bun:test";
import {
  buildFreshBlindResultComment,
  freshBlindResultMarker,
  type FreshBlindPublishedResult,
} from "./publish-fresh-blind-result";

const result = {
  artifactDigest: "a".repeat(64),
  artifactId: "123",
  artifactUrl: "https://github.com/corca-ai/semath/actions/runs/7/artifacts/123",
  candidateSha: "b".repeat(40),
  releaseId: "v0.37",
  reservationSha256: "c".repeat(64),
  runUrl: "https://github.com/corca-ai/semath/actions/runs/7",
  status: "completed",
  terminalReceiptSha256: "d".repeat(64),
} as const satisfies FreshBlindPublishedResult;

describe("fresh blind terminal result publication", () => {
  test("publishes an immutable identity and artifact digest", () => {
    expect(freshBlindResultMarker(result)).toContain(`v0.37:${"b".repeat(40)}:completed:${"d".repeat(64)}`);
    const comment = buildFreshBlindResultComment(result);
    expect(comment).toContain("Terminal receipt SHA-256");
    expect(comment).toContain("Artifact 123 digest");
  });

  test("makes a missing terminal explicitly publication-blocking", () => {
    const comment = buildFreshBlindResultComment({ ...result, status: "execution-error", terminalReceiptSha256: null });
    expect(comment).toContain("reservation remains spent");
    expect(comment).toContain("publication is blocked");
  });

  test("rejects malformed external artifact identities", () => {
    expect(() => buildFreshBlindResultComment({ ...result, artifactId: "zero" })).toThrow("artifact id");
    expect(() => buildFreshBlindResultComment({ ...result, runUrl: "https://example.com/run" })).toThrow("run URL");
  });
});
