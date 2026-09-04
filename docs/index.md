# Semath documentation

Semath is a conservative Rust/WASM analyzer for mathematical Markdown and
LaTeX. It tracks explicit definitions, checks grounded constraints, and
preserves source identity through edits. CorTeX is its first host.

## Start here

- [Supported scope](conservative-analysis.md): supported work and refusal rules.
- [Architecture](architecture.md): data flow, evidence authority, and boundaries.
- [Lessons](lessons.md): mistakes to avoid when changing the analyzer.

Install dependencies with `bun install`, then run `bun run check` and
`awiki lint -r`. Run `bun run quality` on x86_64 Linux for semantic, pack,
fixture, or threshold changes. Build release WASM only on that platform.

## Integrate and maintain

- [Public API](public-api.md): snapshots, queries, revisions, and source ranges.
- [Domain packs](domain-packs.md): declarative vocabulary and typed constraints.
- [Compatibility and release policy](compatibility.md): versioned contracts and
  reproducible release qualification.
- [Capability and test-layer matrix](capability-test-matrix.md): where to test behavior.
- [Performance gates](performance.md): latency, memory, work, and transfer limits.
- [Documentation guide](metadoc.md): organization, linking, and linting.
- [Agent guide](../AGENTS.md): repository map and working rules.

Current plans belong in [GitHub issues](https://github.com/corca-ai/semath/issues).
Issues do not expand the supported product scope.
