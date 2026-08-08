# Semath

Semath is a Rust/WASM library for semantic navigation in scientific Markdown and LaTeX. It is
designed to be embedded by editors such as CorTeX, not to be a standalone web application.

Current `main` provides:

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
- a transport-neutral language server that combines wasmtex LaTeX intelligence
  with Semath selection, navigation, diagnostics, completion, and reviewable rewrites.

Cursor-addressed symbol queries accept both positions inside a symbol and the
caret immediately after it. If another symbol starts at that same UTF-16
offset, the symbol on the right wins.

Version 0.11 unifies the built-in domain-pack schema and expands the calibrated
catalog to linear algebra, probability/statistics, calculus/analysis,
optimization/ML, and discrete mathematics. Broad entries are recognition-only;
typed completion and guarded rewrites remain limited to explicitly proven
linear-algebra and probability cases. See
[the pack contract](./docs/domain-packs.md) and
[capability/test-layer matrix](./docs/capability-test-matrix.md).

Version 0.12 hardens that same five-pack, 68-pattern scope without adding new
mathematical concepts or edit authority. One structure-anchored matcher now
owns every pattern; the calibration gate exercises multiple grouped,
assignment, and delimiter surfaces, structural refusals, and cross-pack
collisions. A realistic seven-file mixed-domain project additionally gates
native/release-WASM parity, lifecycle ordering, parse reuse, latency, response
size, and retained memory.

Version 0.13 closes interaction correctness gaps without adding patterns or UI
authority. One pure cursor policy now owns token starts, interiors, trailing
edges, shared boundaries, gaps, and ambiguity. Definition/reference resolution
respects section visibility and include expansion order, refuses future or
multiply-expanded candidates, and validates change batches before mutation.
The five existing packs retain 68 patterns while adding generated Unicode/CRLF
contexts, mutation refusals, and executable per-pack scorecards.

Version 0.10 adds recoverable application, matrix, cases, and paired-delimiter
IR; project/include/scope-aware semantic symbol identities; bounded macro
provenance through wasmtex syntax schema 2; and a reusable prioritized Worker
host with typed cancellation and failure results. Its multi-file corpus rejects
cross-scope false links and shares native/WASM parity and latency gates.

Version 0.5 adds a bounded `symbolInfo` query that returns the definitions, shape
claims, recognized formulas, diagnostics, and source evidence associated with one symbol. It also
recognizes conservative English definition forms such as `respectively`, apposition,
parenthetical definitions, typed quantifiers, and direct relational statements. Explicit scalar,
vector, matrix, and tensor nouns can supply scoped shape claims with refinements such as
`symmetric`, `diagonal`, or `normalized`. A separate bounded `domainEvidence` query reports the
linear-algebra and probability packs active at the current section or equation. Vocabulary and
notation priors stay weak and cannot create definitions or diagnostics; a typed formula match is
strong evidence only for that equation. Explicit definitions can also contribute scoped semantic
roles such as set/event, function/operator, probability distribution, random variable, and index.
Warnings require incompatible explicit role or shape claims in the same scope and retain every
conflicting source; compatible roles and section-level shadowing are left alone.

Definition hygiene is deliberately quieter. `used-before-explicit-definition` and
`defined-but-unused` are hints only when one complete document has a unique strong definition and
resolved free occurrences in its effective scope. Multiple files or definitions, notation tables,
binders, unfinished math, and convention-only notation disable the hint. The checked-in calibration
corpus has a zero-known-false-positive budget: a counterexample disables the affected rule until it
is represented there. Promotion to warning requires a separate change backed by at least 500
human-labeled candidate sites, at least 99% measured precision, and no scope or binder false
positives.

Version 0.6 adds a typed probability pack for event probability, conditional probability,
expectation, and variance. Recognition requires compatible explicit roles; conditional probability
also requires explicit positive-probability evidence for the conditioning event. Completion is
offered only for an explicitly scalar target, remains revision-checked and review-required, and
does not perform equivalent-form rewrites. The labeled probability corpus has a zero-known-false-
positive budget and includes scope, role mismatch, side-condition, and unfinished-input
suppression cases.

Version 0.7 adds bounded equivalent-form rewrite proposals. The first slice expands a conditional
probability into its definition when the conditioning event is explicitly known to have positive
probability, and also offers Bayes' theorem when both events have that evidence. Rewrites preserve
the exact source range and expected text and are always previewed through the host's
revision-checked review flow; they are never automatic edits or diagnostic quick fixes. When a
closed math region has exactly one recognized rewrite target, its action remains available from
the left-hand side, delimiters, and the position immediately after the formula.

Version 0.8 adds one bounded `inspection` query for editor-facing semantic inspection. A single
snapshot now returns the equation tree and selected node path together with symbol definitions,
references, diagnostics, formula/domain evidence, completion and rewrite proposals, and bound-
variable rename availability. Tree depth, node count, references, diagnostics, and existing claim
collections are capped and report truncation, so hosts do not need to coordinate many independently
timed queries or accept an unbounded sidebar response.

The Rust core is host-independent. The standalone [public API](docs/public-api.md),
[domain-pack contract](docs/domain-packs.md), and [compatibility policy](docs/compatibility.md)
are documented separately from the CorTeX host. Browser artifacts under `lib/wasm` are built on an x86_64
Linux build host; Apple Silicon machines run native tests but must not produce release WASM.
See [draft.md](./draft.md) for the architecture and roadmap.

## Language server

`semath/lsp` is an embeddable JSON-RPC server core. It owns one shared wasmtex
syntax runtime, so LaTeX providers and Semath consume a single parse per document
revision. The package also includes a Bun-based stdio host for native editors:

```sh
bun node_modules/semath/packages/lsp/stdio.mjs
```

Configure the command as a full-sync language server for `latex`, `markdown`, and
`bibtex` documents. It exposes selection ranges, hover, definition, references,
prepare-rename/rename, completion, code actions, diagnostics, and the bounded
`semath/inspection` request. Formula edits remain review-required LSP workspace
edits; the server never changes source on its own.

## Verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bun test
bun packages/packs/conformance.mjs
```

On a supported build host, `scripts/build-wasm.sh` regenerates the checked-in WASM package and
its checksums. From an Apple Silicon development machine, set `SEMATH_BUILD_HOST` to a separate
x86_64 Linux host and run `scripts/build-wasm-remote.sh`; it syncs an isolated source tree, builds,
and retrieves only the artifacts. CI verifies the committed checksums, then independently rebuilds
the package and checks native/WASM behavior parity plus the generated JavaScript and TypeScript ABI.
