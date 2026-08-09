# Semath documentation

Semath is an embeddable Rust/WASM semantic engine for mathematical and
engineering Markdown and LaTeX. CorTeX is its first host, but the core and
public contracts are host-independent.

## Start here

- [Architecture](architecture.md) explains the current durable design and
  boundaries.
- [Public API](public-api.md) describes snapshots, updates, queries, source
  ranges, and result contracts.
- [GitHub issues](https://github.com/corca-ai/semath/issues) contain current
  plans and acceptance criteria.

Install dependencies with `bun install` and run the complete local verification
suite with:

```sh
bun run check
awiki lint -r
```

Release WASM must be built on an x86_64 Linux host. Apple Silicon machines may
run native tests but must use `scripts/build-wasm-remote.sh` for release
artifacts.

## Work with the semantic engine

- [Public API](public-api.md) — embed or query Semath through native, WASM,
  Worker, or language-server boundaries.
- [Domain packs](domain-packs.md) — understand and author concepts, typed laws,
  quantities, and pack dependencies.
- [Compatibility and release policy](compatibility.md) — change schemas,
  protocols, packages, and release artifacts safely.

## Evaluate behavior

- [Capability and test-layer matrix](capability-test-matrix.md) — choose the
  smallest authoritative test layer for each capability.
- [Semantic quality scorecards](semantic-quality-scorecards.md) — interpret and
  change calibration, refusal, corpus, parity, and performance budgets.

## Maintain the project

- [Documentation Guide](metadoc.md) — write, organize, link, and lint docs.
- [`AGENTS.md`](../AGENTS.md) — follow the compact working rules and repository
  map used by coding agents.
- [`draft.md`](../draft.md) — consult the historical proposal and iteration
  record; do not use it as the current architecture or plan.
