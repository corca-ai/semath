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

Semath accepts wasmtex syntax-contract v4. Math regions, includes, project
macros, call-site ranges, expansion provenance, and ambiguity are supplied once
at this boundary. Semath does not rescan TeX to construct a competing syntax
model. Direct syntax, shallow call-site shapes, and range-free generated
notation for bounded composite macro expansions lower through the same
canonical path. Semath never parses the expansion surface string; opaque or
ambiguous expansions are refused.

## Project semantic index

`ProjectSemanticIndex` is the sole project-wide identity, claim, evidence,
candidate, and resolution authority. An analyzed document retains source plus
compact scope, hygiene, canonical, candidate, and immutable observation
projections; the full wasmtex CST is released after analysis. These
observations are colocated with their document, have no independent identity
graph, and are replaced atomically with it. Queries resolve them through the
project index and project one bounded view; no parallel project fact store
exists.

The index has exactly two authoritative identities: a revision-qualified
`SourceOccurrenceId` for real source spans and a scoped `EntityId` anchored to
an occurrence. Notation components are compositional values, not identities.
Typed claims retain explicit polarity, modality, source provenance, extraction
rule version, and strictly increasing derivation tiers. Alias resolution is
scope- and source-order-aware and retracts with its evidence; strings are never
permanently unioned.

Structural ambiguity is represented as bounded candidate claims attached to a
real source occurrence. Candidate construction reads only the wasmtex CST and
is deterministic; it does not assign conventional meaning. Typed claims on the
resolved entity provide supporting or rejecting evidence through an
entity-keyed index. Thus `\operatorname{acc}(B_m)` can carry application and
juxtaposition possibilities without wasmtex or a command switch declaring the
answer, while decorated and styled forms remain distinct from their nucleus.

Included documents contribute the same namespaced role, shape, quantity, unit,
dimension, and evidence records used locally. A per-document external type
environment is derived from include order; facts never flow backwards across an
include site or between disconnected components.

Pure extractors derive document observations from immutable documents. Project mutation,
reverse-include invalidation, cancellation, and caching form the effectful
shell. An edit reanalyzes only the changed document and its reverse include
closure; clean and incremental rebuilds must produce the same semantic result.

English scientific prose follows the same functional-core boundary. Bounded
stages segment visible spans, extract mentions and claim spans, determine each
claim's polarity and modality, and lower it to the same occurrences, entities,
claims, and evidence used by notation. Parenthetical and explicit acronym
forms, glossary/acronym resources, and named-operator declarations therefore
share one resolution path. Non-asserting and negative claims remain
source-linked evidence but cannot establish navigation. Exact notation keys
prevent prose `ECE` from silently merging plain juxtaposition, styled text, and
`\operatorname{ECE}`. Packs classify resulting descriptions through
namespaced concepts; they do not add sentence recognizers.

## Canonical semantic IR

Source spelling and layout remain in the structural representation. The
canonical IR represents a small compositional vocabulary: symbols, numbers,
directional relations, sums, products, fractions, powers, multi-argument
applications, composition, total and partial derivatives with explicit order
and variables, integrals with differentials and bounds when structurally
available, nabla applications, dot and cross products, and explicit set union,
intersection, and membership operators. Indexed occurrences retain base/index
components while their complete surface remains the entity key. Operator
meaning is not encoded as a textbook law name. Every node keeps source ranges
and macro provenance.

Normalization is layered:

1. presentation normalization removes irrelevant TeX styling;
2. structural normalization applies only universally safe equivalences;
3. a typed law may admit further forms only under its declared constraints.

Unknown macros, unresolved roles, incompatible shapes, and contradictory prose
remain uncertainty. They are never filled in by guesswork.

Transparent macro structure supplied by wasmtex may contribute meaning in both
formulas and prose declarations. Generated nodes inherit only the real call
and definition provenance and are never editable locations. Unresolved,
cyclic, truncated, or otherwise opaque calls contribute no semantic tokens; a
command name is not treated as its meaning.

## Domain packs

A domain pack is declarative expert knowledge compiled to one Rust runtime IR.
Schema 7 describes namespaced concepts and reviewed English aliases, typed law
roles, semantic forms, conditions, activation evidence, quantities, units, and
references. Activation evidence is either a reviewed prose phrase or a closed
structural kind derived from the canonical math model; arbitrary pattern
strings are not a runtime policy language. The generic unifier applies every
compiled law; selecting a pack or law in orchestration code is forbidden.

Law conditions use closed kinds and validated role subjects. The runtime
resolves them to bound symbols and source evidence; free-form labels are only a
presentation projection, never a second constraint model.

Pack compilation rejects unknown fields, unresolved or wrong-kind concepts,
invalid dependencies, cycles, and inconsistent dimensions. A build-generated,
sorted catalog discovers versioned pack JSON without a handwritten Rust
registry; the same compiler report is exposed through WASM to authoring tools.
Built-in JSON is compiled once into bounded indexes. There is no second
TypeScript validator or legacy pattern runtime.

Law dispatch compiles root operators and mandatory canonical structure into an
immutable index. Inference sends only structurally compatible candidates to
the generic unifier; a bounded generic bucket exists only for forms such as
variadic balances that have no sound discriminator. The exhaustive scan is a
test oracle, not a production path.

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
Every law role has a semantic concept and may additionally constrain a physical
quantity. These are orthogonal: a calculus variable can carry a duration, while
two different physical quantities remain incompatible.
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

Protocol 7 exposes selection, `semanticView`, definition, references, rename,
and diagnostics. `semanticView.decision` is the sole exhaustive meaning state:
established, partial, ambiguous, conflicting, or unsupported. Summary text is
a projection of that typed decision; it never selects policy. Native, WASM,
Worker, LSP, and CorTeX consume the same result model, and raw parser trees are
not public.

Breaking API or UI changes are acceptable before 1.0 when they remove a
duplicate path or produce a cleaner architecture. Compatibility layers must not
leave old and new semantic runtimes in parallel.

## Invariants

- Rust core behavior is host-independent and native/WASM-equivalent.
- One structural frontend and one project semantic index own their facts.
- `bun run architecture` rejects reverse dependencies, duplicate adapters,
  production compatibility modes, raw-TeX parser leakage, and a parallel
  project fact store.
- `bun run legacy:audit:check` rejects reviewed obsolete protocol fields,
  pack-pattern runtime fields, raw lexical inference paths, and exhaustive
  production dispatch signatures.
- One pack schema/compiler and one generic law runtime are active.
- Every conclusion retains source evidence and bounded uncertainty.
- Pure transformations contain normalization, inference, ranking, and view
  construction wherever practical.
- User interfaces explain decisions and next actions, not internal trees.
- Release WASM is built on an x86_64 Linux host, never on Apple Silicon.
