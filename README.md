# Semath

Semath is a Rust/WASM library for semantic navigation in scientific Markdown and LaTeX. It is
designed to be embedded by editors such as CorTeX, not to be a standalone web application.

Version 0.3 provides:

- structural selection and a queryable equation tree;
- explicit English definition extraction (`Let x denote …`) with hover, definition, and references;
- capture-avoiding rename proposals for variables bound by sums, limits, and quantifiers;
- explicit vector/matrix shapes, conservative propagation, and evidence-backed contradiction diagnostics;
- diagnostic explanation queries without speculative quick fixes;
- UTF-16 source ranges compatible with Monaco;
- a versioned project/update/query protocol and browser Worker runtime;
- an adapter for the public `wasmtex/syntax` snapshot contract.

The Rust core is host-independent. Browser artifacts under `lib/wasm` are built on an x86_64
Linux build host; Apple Silicon machines run native tests but must not produce release WASM.
See [draft.md](./draft.md) for the architecture and roadmap.

## Verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bun test
```

On a supported build host, `scripts/build-wasm.sh` regenerates the checked-in WASM package.
