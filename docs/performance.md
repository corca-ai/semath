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
malformed recovery, and binder/rename notation. A measured leaf edit must parse
and transfer only its own syntax snapshot and analyze only its reverse-include
closure. An empty delta must do no analysis. Clean and incremental summaries
must agree.

Every report separates cold reset, syntax update, total edit latency, each query
kind, peak and retained RSS growth, adapter transfer bytes, syntax nodes and
bytes, affected documents, and the WASM artifact size. These are code defaults,
not live production telemetry. The dependency lock pins the wasmtex input used
for a report; record the Semath and wasmtex commits when comparing reports.

Ordinary CI runs both document counts to catch deterministic scope and transfer
regressions. Release comparisons that enforce percentage changes must run on a
stable x86_64 Linux host; Apple Silicon timings are diagnostic only. Never relax
a threshold merely to make a feature branch pass. If hosted-runner scheduling
noise exceeds a latency gate, reproduce it on the stable host and preserve the
per-layer counters needed to distinguish real work growth from scheduler delay.
Stable-host mode enforces the currently approved leaf-edit p95 limits of 25ms at
61 documents and 50ms at 501 documents. The ordinary hosted-runner gate allows
75ms solely for scheduler jitter; it does not replace the stable release gate.
