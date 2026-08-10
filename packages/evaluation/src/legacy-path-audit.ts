export const LEGACY_SEMANTIC_PATHS = [
  {
    id: "raw-law-prose-window",
    ownerIssue: 189,
    path: "crates/semath-core/src/law.rs",
    markers: ["fn sentence_around", "document.content"],
  },
  {
    id: "raw-law-lexical-policy",
    ownerIssue: 189,
    path: "crates/semath-core/src/law.rs",
    markers: ["fn context_supports_law", "contradicted"],
  },
  {
    id: "raw-domain-lexical-scan",
    ownerIssue: 189,
    path: "crates/semath-core/src/domain.rs",
    markers: ["fn collect_priors", "literal_matcher"],
  },
  {
    id: "exhaustive-law-dispatch",
    ownerIssue: 190,
    path: "crates/semath-core/src/law.rs",
    markers: ["for actual in canonical_expressions", "for compiled in COMPILED_LAWS.iter()"],
  },
  {
    id: "notation-only-law-role-fallback",
    ownerIssue: 193,
    path: "crates/semath-core/src/law.rs",
    markers: ["fn notation_matches", "role.notation"],
  },
  {
    id: "inline-meaning-decision",
    ownerIssue: 191,
    path: "crates/semath-core/src/engine.rs",
    markers: ["let (status, summary, refusal) = if conflicting"],
  },
  {
    id: "stale-protocol-documentation",
    ownerIssue: 188,
    path: "docs/public-api.md",
    markers: ["Protocol 4"],
  },
  {
    id: "stale-pack-maturity-protocol",
    ownerIssue: 193,
    path: "docs/pack-maturity.md",
    markers: ["Protocol 4"],
  },
  {
    id: "legacy-typescript-view-state",
    ownerIssue: 193,
    path: "packages/protocol/src/index.ts",
    scopeStart: "export interface SemanticViewInfo {",
    scopeEnd: "\n}",
    markers: ["refusal?: string;", "status:", "summary: string;"],
  },
  {
    id: "legacy-rust-view-state",
    ownerIssue: 193,
    path: "crates/semath-core/src/protocol.rs",
    scopeStart: "pub struct SemanticViewInfo {",
    scopeEnd: "\n}",
    markers: ["pub refusal:", "pub status:", "pub summary:"],
  },
  {
    id: "legacy-pack-patterns-field",
    ownerIssue: 193,
    path: "crates/semath-core/src/pack.rs",
    scopeStart: "pub struct DomainPack {",
    scopeEnd: "\n}",
    markers: ["pub patterns:"],
  },
  {
    id: "legacy-corpus-expectation",
    ownerIssue: 193,
    path: "packages/evaluation/src/model.ts",
    markers: ['CorpusExpectation = "established"'],
  },
  {
    id: "legacy-corpus-case-name",
    ownerIssue: 193,
    path: "packages/evaluation/src/model.ts",
    markers: ["EstablishedCorpusCase"],
  },
  {
    id: "legacy-corpus-observation-name",
    ownerIssue: 193,
    path: "packages/evaluation/src/scorecard.ts",
    markers: ["establishedLawIds"],
  },
] as const;

export type LegacySemanticPathId = (typeof LEGACY_SEMANTIC_PATHS)[number]["id"];

export interface LegacySemanticPathFinding {
  readonly id: LegacySemanticPathId;
  readonly ownerIssue: number;
  readonly path: string;
}

export function auditLegacySemanticPaths(
  sources: Readonly<Record<string, string | undefined>>,
): readonly LegacySemanticPathFinding[] {
  return LEGACY_SEMANTIC_PATHS.flatMap((rule) => {
    const source = sources[rule.path];
    const scope = source === undefined ? undefined : auditScope(source, rule);
    return scope !== undefined && rule.markers.every((marker) => scope.includes(marker))
      ? [{ id: rule.id, ownerIssue: rule.ownerIssue, path: rule.path }]
      : [];
  });
}

function auditScope(
  source: string,
  rule: (typeof LEGACY_SEMANTIC_PATHS)[number],
): string | undefined {
  if (!("scopeStart" in rule)) return source;
  const start = source.indexOf(rule.scopeStart);
  if (start < 0) return undefined;
  const end = source.indexOf(rule.scopeEnd, start + rule.scopeStart.length);
  return end < 0 ? undefined : source.slice(start, end);
}
