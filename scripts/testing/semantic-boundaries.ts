export const FORBIDDEN_SEMANTIC_PATHS = [
  {
    id: "raw-law-prose-window",
    path: "crates/semath-core/src/law.rs",
    markers: ["fn sentence_around", "document.content"],
  },
  {
    id: "raw-law-lexical-policy",
    path: "crates/semath-core/src/law.rs",
    markers: ["fn context_supports_law", "contradicted"],
  },
  {
    id: "raw-domain-lexical-scan",
    path: "crates/semath-core/src/domain.rs",
    markers: ["fn collect_priors", "literal_matcher"],
  },
  {
    id: "exhaustive-law-dispatch",
    path: "crates/semath-core/src/law.rs",
    markers: ["for actual in canonical_expressions", "for compiled in COMPILED_LAWS.iter()"],
  },
  {
    id: "notation-only-law-role-fallback",
    path: "crates/semath-core/src/law.rs",
    markers: ["fn notation_matches", "role.notation"],
  },
  {
    id: "inline-meaning-decision",
    path: "crates/semath-core/src/engine.rs",
    markers: ["let (status, summary, refusal) = if conflicting"],
  },
  {
    id: "forbidden-typescript-view-state",
    path: "packages/protocol/src/index.ts",
    scopeStart: "export interface SemanticViewInfo {",
    scopeEnd: "\n}",
    markers: ["refusal?: string;", "status:", "summary: string;"],
  },
  {
    id: "forbidden-rust-view-state",
    path: "crates/semath-core/src/protocol.rs",
    scopeStart: "pub struct SemanticViewInfo {",
    scopeEnd: "\n}",
    markers: ["pub refusal:", "pub status:", "pub summary:"],
  },
  {
    id: "forbidden-decision-summary-policy-ts",
    path: "packages/protocol/src/index.ts",
    markers: ["summary: string;"],
  },
  {
    id: "forbidden-decision-missing-policy-ts",
    path: "packages/protocol/src/index.ts",
    markers: ["missing: readonly MeaningRequirement[];"],
  },
  {
    id: "forbidden-decision-summary-policy-rust",
    path: "crates/semath-core/src/protocol.rs",
    markers: ["summary: String,"],
  },
  {
    id: "forbidden-decision-missing-policy-rust",
    path: "crates/semath-core/src/protocol.rs",
    markers: ["missing: Vec<MeaningRequirement>"],
  },
  {
    id: "forbidden-pack-patterns-field",
    path: "crates/semath-core/src/pack.rs",
    scopeStart: "pub struct DomainPack {",
    scopeEnd: "\n}",
    markers: ["pub patterns:"],
  },
  {
    id: "mutually-exclusive-clause-disposition",
    path: "crates/semath-core/src/scientific_prose.rs",
    markers: ["ClauseDisposition"],
  },
  {
    id: "sentence-regex-definition-runtime",
    path: "crates/semath-core/src/prose.rs",
    markers: ["COORDINATED_MAPPING_SUFFIX"],
  },
  {
    id: "raw-tex-citation-policy",
    path: "crates/semath-core/src/scientific_prose.rs",
    markers: ["contains(\"\\\\cite\")"],
  },
] as const;

export type SemanticBoundaryId = (typeof FORBIDDEN_SEMANTIC_PATHS)[number]["id"];

export interface SemanticBoundaryFinding {
  readonly id: SemanticBoundaryId;
  readonly path: string;
}

export function checkSemanticBoundaries(
  sources: Readonly<Record<string, string | undefined>>,
): readonly SemanticBoundaryFinding[] {
  return FORBIDDEN_SEMANTIC_PATHS.flatMap((rule) => {
    const source = sources[rule.path];
    const scope = source === undefined ? undefined : auditScope(source, rule);
    return scope !== undefined && rule.markers.every((marker) => scope.includes(marker))
      ? [{ id: rule.id, path: rule.path }]
      : [];
  });
}

function auditScope(
  source: string,
  rule: (typeof FORBIDDEN_SEMANTIC_PATHS)[number],
): string | undefined {
  if (!("scopeStart" in rule)) return source;
  const start = source.indexOf(rule.scopeStart);
  if (start < 0) return undefined;
  const end = source.indexOf(rule.scopeEnd, start + rule.scopeStart.length);
  return end < 0 ? undefined : source.slice(start, end);
}
