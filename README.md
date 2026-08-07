# Semath

Semath is a Rust/WASM library for semantic navigation in scientific Markdown and LaTeX. It is
designed to be embedded by editors such as CorTeX, not to be a standalone web application.

Version 0.4 provides:

- structural selection and a queryable equation tree;
- explicit English definition extraction (`Let x denote …`) with hover, definition, and references;
- capture-avoiding rename proposals for variables bound by sums, limits, and quantifiers;
- explicit vector/matrix shapes, conservative propagation, and evidence-backed contradiction diagnostics;
- scoped scalar, vector, matrix, and tensor facts with local shadowing;
- a versioned linear-algebra pack shared by bounded formula recognition and typed completion;
- review-required formula edit proposals for matrix/vector products, transpose, inner products, and quadratic forms;
- diagnostic explanation queries without speculative quick fixes;
- UTF-16 source ranges compatible with Monaco;
- a versioned project/update/query protocol and browser Worker runtime;
- an adapter for the public `wasmtex/syntax` snapshot contract.

The v0.5 work in progress adds a bounded `symbolInfo` query that returns the definitions, shape
claims, recognized formulas, diagnostics, and source evidence associated with one symbol. It also
recognizes conservative English definition forms such as `respectively`, apposition,
parenthetical definitions, typed quantifiers, and direct relational statements. Explicit scalar,
vector, matrix, and tensor nouns can supply scoped shape claims with refinements such as
`symmetric`, `diagonal`, or `normalized`. A separate bounded `domainEvidence` query reports the
linear-algebra and probability packs active at the current section or equation. Vocabulary and
notation priors stay weak and cannot create definitions or diagnostics; a typed formula match is
strong evidence only for that equation.

The Rust core is host-independent. Browser artifacts under `lib/wasm` are built on an x86_64
Linux build host; Apple Silicon machines run native tests but must not produce release WASM.
See [draft.md](./draft.md) for the architecture and roadmap.

## Verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bun test
```

On a supported build host, `scripts/build-wasm.sh` regenerates the checked-in WASM package and
its checksums. CI verifies those committed checksums, then independently rebuilds the package and
checks native/WASM behavior parity plus the generated JavaScript and TypeScript ABI.
