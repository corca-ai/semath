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

Install dependencies with `bun install` and run the fast local verification
suite with:

```sh
bun run check
awiki lint -r
```

`bun run check` is the fast code gate used by pull requests. Expensive semantic
corpus evaluation is intentionally manual; run `bun run quality` for semantic
releases and changes to packs, corpora, inference, or quality thresholds.

Release WASM must be built on an x86_64 Linux host. Apple Silicon machines may
run native tests but must use `scripts/build-wasm-remote.sh` for release
artifacts.

## Work with the semantic engine

- [Public API](public-api.md) — embed or query Semath through native, WASM,
  Worker, or language-server boundaries.
- [Domain packs](domain-packs.md) — understand and author concepts, typed laws,
  quantities, and pack dependencies.
- [Pack maturity report](pack-maturity.md) — inspect dated benchmark evidence,
  resolved defects, and measured gaps without confusing them with rollout state.
- [Compatibility and release policy](compatibility.md) — change schemas,
  protocols, packages, and release artifacts safely.

## Evaluate behavior

- [Capability and test-layer matrix](capability-test-matrix.md) — choose the
  smallest authoritative test layer for each capability.
- [Semantic quality scorecards](semantic-quality-scorecards.md) — interpret and
  change calibration, refusal, corpus, parity, and performance budgets.
- [Spent holdout postmortems](spent-holdout-postmortems.md) — understand the
  historical v0.38/v0.39 failures, public regression atlas, and reuse boundary.
- [Practical STEM breadth benchmark](stem-breadth-benchmark.md) — interpret the
  reviewed field-by-capability development matrix and its commissioned gaps.
- Inspect the dated cursor, identity, navigation, and lifecycle fixture review in
  the [v0.28 development-contract adjudication](v028-development-adjudication.md).
- [Performance gates](performance.md) — reproduce full-path latency, memory,
  transfer, artifact, and bounded-invalidation measurements.

## Maintain the project

- [Documentation Guide](metadoc.md) — write, organize, link, and lint docs.
- [`AGENTS.md`](../AGENTS.md) — follow the compact working rules and repository
  map used by coding agents.
