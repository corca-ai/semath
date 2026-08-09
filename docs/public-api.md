# Public API

Semath is a library and language-service runtime, not an editor or web application. Public imports are split by ownership boundary:

| Export | Use |
| --- | --- |
| `semath/protocol` | Versioned snapshots, changes, queries, results, diagnostics, inspections, and reviewed edit proposals |
| `semath/wasm` | Generated release WASM initialization and the low-level byte-oriented engine ABI |
| `semath/worker` | Typed engine wrapper around the generated WASM ABI |
| `semath/worker-host` | Browser Worker scheduling, recovery, and stale-generation fencing |
| `semath/worker-runtime` | Worker-side request dispatcher |
| `semath/wasmtex-adapter` | Pure conversion from wasmtex syntax documents to Semath project documents |
| `semath/lsp` | Transport-neutral LSP server with standard methods and `semath/inspection` |
| `semath/packs` | Immutable built-in pack metadata plus the pure pack loader and validator |

`semath-pack` validates the bundled catalog or caller-supplied JSON without loading CorTeX. `semath-lsp` provides the stdio server. The executable [Worker](../examples/worker.mjs) and [LSP](../examples/lsp.mjs) examples use only these public exports.

Semantic parsing, evidence, identity, and edit proposals belong to the Rust core. Host packages own transport and lifecycle policy. Applications own presentation, permissions, review, apply, and undo; they must not apply a semantic edit without review and revision validation.

Runtime loading of arbitrary third-party pack code is intentionally unsupported. The public pack API validates declarative metadata against the bounded built-in primitive registry.

`semanticContext` returns a bounded, evidence-bearing projection for the symbol
or formula under the cursor: namespaced concepts, claims and conflict status,
law relations with bound roles, and quantity/unit/dimension facts. The same
projection is included as optional `inspection.semantic`, so hosts can render a
meaning-first view without coordinating another query. These additions are
protocol-v1-compatible optional data; consumers must ignore fields they do not use.
