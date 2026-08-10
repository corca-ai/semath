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
    return source !== undefined && rule.markers.every((marker) => source.includes(marker))
      ? [{ id: rule.id, ownerIssue: rule.ownerIssue, path: rule.path }]
      : [];
  });
}
