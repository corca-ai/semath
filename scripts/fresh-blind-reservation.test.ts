import { describe, expect, test } from "bun:test";
import {
  assertFreshBlindLedgerComment,
  assertFreshBlindIdentityAvailable,
  freshBlindReservationMarker,
  parseFreshBlindLedgerComment,
  type FreshBlindReservationIdentity,
} from "./fresh-blind-reservation";

const identity = {
  candidateSha: "c".repeat(40),
  fixtureSeal: "f".repeat(64),
  releaseId: "v0.37",
  runAttempt: "1",
  runId: "1234",
} as const satisfies FreshBlindReservationIdentity;

describe("fresh blind global reservation", () => {
  test("serializes one exact release and fixture identity", () => {
    expect(freshBlindReservationMarker(identity)).toBe(
      `<!-- semath-fresh-blind-reservation:v0.37:${"f".repeat(64)}:${"c".repeat(40)} -->`,
    );
    expect(() =>
      assertFreshBlindIdentityAvailable(["ordinary comment"], identity),
    ).not.toThrow();
  });

  test("rejects reuse through either a release id or fixture seal", () => {
    const marker = freshBlindReservationMarker(identity);
    expect(() =>
      assertFreshBlindIdentityAvailable([marker], {
        ...identity,
        candidateSha: "d".repeat(40),
        fixtureSeal: "a".repeat(64),
      }),
    ).toThrow("release id is already spent");
    expect(() =>
      assertFreshBlindIdentityAvailable([marker], {
        ...identity,
        releaseId: "v0.38",
      }),
    ).toThrow("fixture seal is already spent");
  });

  test("rejects every historical spent and terminal release marker", () => {
    expect(() =>
      assertFreshBlindIdentityAvailable(
        ["<!-- semath-fresh-blind:v0.37:spent -->"],
        identity,
      ),
    ).toThrow("release id is already spent");
    expect(() =>
      assertFreshBlindIdentityAvailable(
        [
          `<!-- semath-fresh-blind-result:v0.37:${"c".repeat(40)}:execution-error:none -->`,
        ],
        identity,
      ),
    ).toThrow("release id is already spent");
  });

  test("binds the local reservation to the official bot-authored ledger comment", () => {
    const marker = freshBlindReservationMarker(identity);
    const comment = parseFreshBlindLedgerComment({
      body: marker,
      created_at: "2026-08-20T08:00:00Z",
      html_url:
        "https://github.com/corca-ai/semath/issues/354#issuecomment-123",
      id: 123,
      issue_url: "https://api.github.com/repos/corca-ai/semath/issues/354",
      user: { login: "github-actions[bot]" },
    });
    expect(() =>
      assertFreshBlindLedgerComment(comment, {
        issue: "354",
        marker,
        repository: "corca-ai/semath",
      }),
    ).not.toThrow();
    expect(() =>
      assertFreshBlindLedgerComment(
        { ...comment, author: "untrusted-user" },
        { issue: "354", marker, repository: "corca-ai/semath" },
      ),
    ).toThrow("GitHub Actions");
  });

  test("rejects malformed identities before any external effect", () => {
    expect(() =>
      freshBlindReservationMarker({ ...identity, candidateSha: "short" }),
    ).toThrow("candidate SHA");
  });
});
