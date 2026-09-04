# Capability and test-layer matrix

Tests specify the [supported scope](conservative-analysis.md). The lowest layer
that can state a behavior owns its permutations. Boundary checks use the real
syntax adapter and shipped engine; passing native helpers alone is insufficient.

| Capability | Authoritative check | Boundary check |
| --- | --- | --- |
| Syntax, UTF-16 ranges, malformed input | wasmtex contract and notation conformance | adapter tests and native/WASM parity |
| Definition, reference, and cursor identity | Rust scope, binder, source-order, and hygiene tests | Markdown/TeX acceptance and LSP mapping |
| Rename | Rust complete-set, capture, generated-source, and fanout refusal tests | real rename acceptance and Worker/LSP mapping |
| Shape, dimension, and unit consistency | Rust typed-operation and evidence tests | foundation fixtures and positive/negative acceptance |
| Formula evidence and uncertainty | Rust source-grounding and unsupported-input tests | formula-root acceptance and cursor parity |
| Pack compilation | Rust compiler, unifier, and dependency tests | `bun run pack:authoring` and package smoke |
| Incremental analysis | Rust retraction and dependency tests | full lifecycle parity and clean rebuild comparison |
| Worker lifecycle | queue, generation, and transaction tests | engine reset/recreation and package examples |
| Performance bounds | pure policy and bounded-work tests | 61/501-document measurements on x86_64 Linux |

`bun run check` runs local code and supported-behavior checks. `bun run quality`
adds the foundation regressions, complete lifecycle traces, and stable repeated
performance measurements. [Release qualification](compatibility.md) records the
exact tested source and artifacts.

Keep examples small and independently understandable. Every diagnostic needs a
corresponding grounded case and a missing-evidence case. Changes to evidence
handling must preserve retraction and native/WASM parity. Do not replace useful
positive cases with silence merely to make a check pass.

Editor E2E tests belong to the host and should cover wiring risks that cannot be
expressed below the UI. Semantic permutations belong here.
