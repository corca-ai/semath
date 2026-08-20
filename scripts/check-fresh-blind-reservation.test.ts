import { describe, expect, test } from "bun:test";
import { parseFreshBlindReservation } from "./check-fresh-blind-reservation";
import { freshBlindReservationMarker } from "./fresh-blind-reservation";

const identity = {
  candidateSha: "a".repeat(40),
  fixtureSeal: "b".repeat(64),
  releaseId: "v0.37",
  runAttempt: "1",
  runId: "123",
} as const;

function reservation() {
  return {
    ...identity,
    ledgerCommentId: "456",
    marker: freshBlindReservationMarker(identity),
    reservedAt: "2026-08-20T08:00:00.000Z",
    schemaVersion: 1 as const,
  };
}

describe("fresh blind reservation parser", () => {
  test("accepts the exact durable reservation envelope", () => {
    expect(parseFreshBlindReservation(reservation())).toEqual(reservation());
  });

  test("rejects identity changes even when fields remain well formed", () => {
    expect(() =>
      parseFreshBlindReservation({
        ...reservation(),
        candidateSha: "c".repeat(40),
      }),
    ).toThrow("marker does not match");
  });

  test("rejects unknown fields and malformed timestamps", () => {
    expect(() =>
      parseFreshBlindReservation({ ...reservation(), extra: true }),
    ).toThrow("unexpected or missing fields");
    expect(() =>
      parseFreshBlindReservation({ ...reservation(), reservedAt: "yesterday" }),
    ).toThrow("timestamp");
  });
});
