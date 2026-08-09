# Architecture

Semath is an embeddable Rust/WASM semantic engine for mathematical and
engineering documents. Hosts such as CorTeX own editing and presentation;
Semath owns deterministic semantic analysis. It is not a web application,
computer algebra system, or theorem prover.

## Pipeline

```text
project snapshot or delta
  → wasmtex structural syntax and source provenance
  → bindings, scopes, and ProjectSemanticIndex
  → canonical semantic IR
  → typed, bounded inference over compiled domain packs
  → semantic views, navigation, diagnostics, and reviewed edits
  → native, WASM, Worker, and LSP boundaries
```

Each stage has one contract. wasmtex determines what source structure exists;
Semath determines what that structure can safely mean; the host decides how to
present and act on the result.

## Structural boundary

Semath accepts wasmtex syntax-contract v3. Math regions, includes, project
macros, call-site ranges, expansion provenance, and ambiguity are supplied once
at this boundary. Semath does not rescan TeX to construct a competing syntax
model. Direct, one-level, and bounded composed macro expansions lower through
the same canonical path; opaque or ambiguous expansions are refused.

## Project semantic index

`ProjectSemanticIndex` is the sole persistent owner of semantic facts. Parsed
documents retain source, structural syntax, scopes, and hygiene state, while
the index owns definitions, roles, shapes, quantities, domains, and law
recognitions. Queries project bounded views from that store instead of building
parallel fact graphs.

Included documents contribute the same namespaced role, shape, quantity, unit,
dimension, and evidence records used locally. A per-document external type
environment is derived from include order; facts never flow backwards across an
include site or between disconnected components.

Pure extractors derive fact stores from immutable documents. Project mutation,
reverse-include invalidation, cancellation, and caching form the effectful
shell. An edit reanalyzes only the changed document and its reverse include
closure; clean and incremental rebuilds must produce the same semantic result.

## Canonical semantic IR

Source spelling and layout remain in the structural representation. The
canonical IR represents a small compositional vocabulary: symbols, numbers,
directional relations, sums, products, fractions, powers, multi-argument
applications, composition, derivatives, dot and cross products, and explicit
set union, intersection, and membership operators. Operator meaning is not
encoded as a textbook law name. Every node keeps source ranges and macro
provenance.

Normalization is layered:

1. presentation normalization removes irrelevant TeX styling;
2. structural normalization applies only universally safe equivalences;
3. a typed law may admit further forms only under its declared constraints.

Unknown macros, unresolved roles, incompatible shapes, and contradictory prose
remain uncertainty. They are never filled in by guesswork.

Transparent macro surfaces supplied by wasmtex may contribute meaning in both
formulas and prose declarations. Unresolved, cyclic, truncated, or otherwise
opaque calls contribute no semantic tokens; a command name is not treated as
its meaning.

## Domain packs

A domain pack is declarative expert knowledge compiled to one Rust runtime IR.
Schema 4 describes namespaced concepts, typed law roles, semantic forms,
conditions, activation evidence, quantities, units, and references. The generic
unifier applies every compiled law; selecting a pack or law in orchestration
code is forbidden.

Pack compilation rejects unknown fields, unresolved or wrong-kind concepts,
invalid dependencies, cycles, and inconsistent dimensions. A build-generated,
sorted catalog discovers versioned pack JSON without a handwritten Rust
registry; the same compiler report is exposed through WASM to authoring tools.
Built-in JSON is compiled once into bounded indexes. There is no second
TypeScript validator or legacy pattern runtime.

Authoring keeps a functional core and an effectful shell. Rust owns schema,
catalog, dependency, canonical-form, unit, and dimension validation. Pure
TypeScript functions plan reviewed probe cells, quality runs, explanations,
runtime-branch audits, and per-metric baseline diffs. The CLI alone reads,
writes, and packages files. Existing packs and external authors use this same
boundary in CI; generated reports are artifacts rather than maintained docs.

Surface language patterns remain adapters for declarations and evidence. They
do not encode formula laws. Adding a new law should normally require pack data
and independently authored positive and negative corpus cases, not Rust
recognizer changes.

## Inference and safety

Inference unifies canonical expressions with typed roles, then checks explicit
shape, quantity, role, scope, and condition evidence. Candidate and result
counts are capped. Conclusions preserve their derivation and source ranges.
Runtime concepts always use the pack-qualified identity
`<namespace>:<concept>`; display labels are derived views, never an alternate
identity system. Longer pack vocabulary terms take precedence over embedded
generic terms, while equal-specificity collisions remain unresolved.

The public meaning states are `established`, `partial`, `ambiguous`,
`conflicting`, and `unsupported`. Only deterministic results with verified side
conditions may become reviewed edits. Semath does not prove unrestricted
equivalence, infer arbitrary unstated intent, run pack code, or silently change
units and coordinate frames.

## Public boundary

Protocol 3 exposes selection, `semanticView`, definition, references, rename,
and diagnostics. `semanticView` is meaning-first: summary, roles, conditions,
evidence, declarations, conflicts, and refusal are public; raw parser trees are
not. Native, WASM, Worker, LSP, and CorTeX consume the same result model.

Breaking API or UI changes are acceptable before 1.0 when they remove a
duplicate path or produce a cleaner architecture. Compatibility layers must not
leave old and new semantic runtimes in parallel.

## Invariants

- Rust core behavior is host-independent and native/WASM-equivalent.
- One structural frontend and one project semantic index own their facts.
- One pack schema/compiler and one generic law runtime are active.
- Every conclusion retains source evidence and bounded uncertainty.
- Pure transformations contain normalization, inference, ranking, and view
  construction wherever practical.
- User interfaces explain decisions and next actions, not internal trees.
- Release WASM is built on an x86_64 Linux host, never on Apple Silicon.
