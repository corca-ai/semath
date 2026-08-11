# Performance gates

Semath measures the complete authoring path: wasmtex syntax construction,
adapter encoding, Worker-host scheduling, JS/WASM transfer, semantic update, and
cursor queries. CorTeX DOM rendering is intentionally outside this core gate.

Run the normal 61-document gate, the 501-document scale gate, or write a local
JSON report with:

```sh
bun run budget
bun run budget:scale
SEMATH_BUDGET_STABLE=1 bun run budget:stable
mkdir -p .artifacts && bun run budget:report
```

The fixture set deterministically rotates through the reported ECE expression,
nested modifiers and styles, dense matrices, Unicode and combining characters,
malformed recovery, binder/rename notation, sectioned multi-equation reports,
citation-heavy combined discourse features, construction-heavy coordinated prose,
and scoped malformed neighbors. A measured leaf edit must parse
and transfer only its own syntax snapshot. Append-only comments must do no
semantic analysis; a separate real notation mutation must analyze only its
reverse-include closure and complete within 50ms. An empty delta must also do no
analysis. Clean and incremental summaries must agree.

Every report separates syntax construction, adapter lowering, engine
decode/ingestion, total cold reset, syntax update, total edit latency, and each
query kind. Cold timing includes WebAssembly module initialization. RSS growth
starts after module initialization so host-specific compiled-code caches are not
misreported as document or editor state; it records syntax and engine growth
separately and collects unreachable buffers between timed edit lifecycles. The
report also records peak/retained RSS growth, initial and delta transfer
bytes, CST nodes/recovery/bytes per document and per node, invalidated
documents, semantic nodes/rules visited, and the WASM artifact size. These are
augmented by occurrence, entity, claim, evidence, dependency-edge, and
invalidation counters from the authoritative semantic index. These are code
defaults, not live production telemetry. The dependency lock pins the
wasmtex input used for a report; record the Semath and wasmtex commits when
comparing reports.

`AnalysisStats` also reports segmented prose clauses, accepted construction
candidates, bounded matcher work, domain hypotheses/evidence, and structural
frontier/latent candidate work. These counters make prose-coverage growth
reviewable independently from wall-clock noise and prevent a larger construction
inventory from hiding unbounded matching work.

The report also names every lifecycle family, exposes semantic-view p95
separately from other cursor queries, and records deterministic failure-shrinker
input, output, and evaluation counts. Shrinking has a linear work budget; timing
is not gated on a shared runner.

Lifecycle parity explicitly adds and retracts citation attribution, hedging,
conditions, negation, declarations, macros, and include visibility. Every stage
must match a clean rebuild and restoration must return to the exact prior semantic
projection.

The normal and scale gates cap law candidates at 24 visited rules per document.
This is a structural dispatch budget, independent of installed pack count.
Pure 100-pack and 500-pack fixtures include collision-heavy structural families,
not only unique operators. Domain ordering must place supported candidates in
the bounded frontier without losing any result accepted by the exhaustive test
oracle; genuine identical alternatives remain ambiguous. Full challenge and
corpus execution stays manual, while deterministic counters and caps remain in
ordinary gates.

Ordinary CI runs both document counts to catch deterministic scope, transfer,
memory, and artifact regressions. It enforces latency on the smaller fixture,
but only reports 501-document timings because a shared runner is not a stable
performance host. Release comparisons and every 501-document absolute latency
gate run with `SEMATH_BUDGET_STABLE=1` on a stable x86_64 Linux host; Apple
Silicon timings are diagnostic only. Never relax a threshold merely to make a
feature branch pass. Stable-host mode enforces the currently approved leaf-edit
p95 limits of 25ms at 61 documents and 50ms at 501 documents, plus the existing
cold, semantic-edit, and cursor-query limits. The ordinary hosted-runner gate
allows 75ms at 61 documents solely for scheduler jitter; it does not replace the
stable release gate.
