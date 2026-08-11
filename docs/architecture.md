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

Semath accepts wasmtex syntax contract v8. Math regions, neutral source-order blocks,
document-field and citation
annotations, includes, project
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

The syntax projection has three independent consumers. `ScopeGraph` owns
visibility lifetimes and structural shadowing; a bounded attachment graph owns
which neighboring blocks may exchange discourse evidence; scoped domain
hypotheses only order candidates. A paragraph alone is not a lifetime boundary,
and none of these projections may substitute for another. Repeated relational
paragraphs form neutral equation clusters without changing wasmtex's syntax
contract.

Claims use a closed value algebra for concepts, roles, types, shapes,
dimensions, units, quantity kinds, conditions, relations, and bounded literals.
Predicate/value compatibility is validated at the index boundary. Presentation
strings are projections of these values and are never parsed back into semantic
facts.

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
When an external declaration relinks a wasmtex snapshot, Semath accepts the
same text revision only if the source identity is unchanged and the structural
fingerprint differs. This retracts generated meaning without admitting stale or
rewritten text under an old document version.

English scientific prose follows the same functional-core boundary. A bounded
normalizer emits one source-ordered `ProseEvent` stream containing clause
boundaries, math mentions, declarative lemma classes, description spans,
coordination, connectives, anaphora, and discourse features. Construction
composition consumes that stream and bounded attachment edges; it does not
rescan the document for sentence-shaped templates. Its typed discourse frame
keeps communicative act, polarity, modality, attribution, and conditionality
independent, with exact evidence for every detected feature. Recognition,
attachment, and establishment are separate pure steps: only positive, asserted,
unconditional author claims establish meaning. Cited, hedged, hypothetical,
alternative, ambiguous-anaphoric, and negative observations remain
source-grounded without promotion.
Parenthetical and explicit acronym
forms, glossary/acronym resources, and named-operator declarations therefore
share one resolution path. Non-asserting and negative claims remain
source-linked evidence but cannot establish navigation. Exact notation keys
prevent prose `ECE` from silently merging plain juxtaposition, styled text, and
`\operatorname{ECE}`. Packs classify resulting descriptions through
namespaced concepts; they do not add sentence recognizers.

## Canonical semantic IR

Source spelling and layout remain in the structural representation. The
canonical IR represents a small orthogonal vocabulary: symbols, numbers,
directional relations, sums, products, fractions, powers, multi-argument
applications, explicit conditions, structural indexes, binders, systems, and
piecewise branches. Total and partial derivatives retain explicit variables
and order; integrals retain their differential, body, and independent bounds.
Operators and bound variables are source references with exact ranges and
macro provenance, not lossy strings. Indexed expressions retain base and index
children instead of folding them into a symbol name. Textbook concepts remain
typed operator or pack meaning rather than dedicated IR variants.

Complete and malformed structures use the same lowering path for direct and
transparent macro notation. Missing differentials, incomplete binders, and
opaque children remain explicit unknowns. Systems and cases are built from
wasmtex alignment/environment structure, not by reparsing presentation TeX.

Normalization is layered:

1. presentation normalization removes irrelevant TeX styling;
2. structural normalization applies only universally safe equivalences;
3. a typed law may admit further forms only under its declared constraints.

Unknown meaning, unresolved roles, incompatible shapes, and contradictory prose
remain uncertainty. Equation metadata such as labels and tags is excluded from
the relation while its source range remains available for discourse attachment.
These gaps are never filled in by guesswork.

Transparent macro structure supplied by wasmtex may contribute meaning in both
formulas and prose declarations. Generated nodes inherit only the real call
and definition provenance and are never editable locations. Opaque generated
notation and cyclic or truncated expansions produce an explicit engine-limit
decision; an ordinary command's macro-resolution status alone does not make it
opaque, and a command name is never treated as its expansion meaning.

## Domain packs

A domain pack is declarative expert knowledge compiled to one Rust runtime IR.
Schema 8 describes namespaced concepts and reviewed English aliases, typed law
roles, one canonical relation with optional surface representations, conditions,
activation evidence, quantities, units, and references. Activation evidence is
either a reviewed prose phrase or a closed
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

The compiler also derives two reviewable artifacts from the same catalog. A
domain signature contains normalized terms with their originating concept,
quantity, role, law, or activation declaration. A collision atlas groups
canonical and guarded law forms that can share structure and states which
independent domain, concept, quantity, shape, or condition evidence can
distinguish them. Neither artifact duplicates pack facts or introduces a
pack-specific runtime branch.

Law dispatch compiles root operators, operand shapes, and mandatory canonical
structure into an immutable index. Inference sends only structurally compatible candidates to
the generic unifier; a bounded generic bucket exists only for forms such as
variadic balances that have no sound discriminator. The exhaustive scan is a
test oracle, not a production path.

## Scoped domain routing

Domain relevance is an explainable routing hypothesis, never semantic proof.
One pure transformation combines positive asserted body evidence, complete
title or keyword fields, structural section names, and independently recognized
equations at project/document, section, or equation scope. Author fields,
citations, comments, hedges, conditions, alternatives, and negation cannot
establish a domain. Results use the ordinal tiers `explicit`, `supported`, and
`tentative`; there is no opaque confidence score.

Capability packs remain universal. Structural dispatch opens a bounded
candidate frontier, then scoped hypotheses order field and application packs.
An absent domain never removes a structurally compatible law, produces a
diagnostic, or establishes meaning. Genuine collisions therefore remain
bounded alternatives, in relevance order, until independent typed evidence
resolves or contradicts them. Law-derived equation relevance is computed only
after law matching and cannot feed back to activate the same law.

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
generic terms, while equal-specificity collisions remain unresolved. Concept
titles, reviewed aliases, and semantic role IDs may classify prose; descriptive
law-role sentences do not become unreviewed global aliases.

Each law owns one `canonicalRelation`; optional `representations` describe only
surface forms. A pure, bounded equivalence compiler derives equality
orientation, declared-scalar permutations, factor isolation, reciprocal
normalization, and constant-denominator scaling. It emits proof steps and
explicit nonzero guards with every derived form. It never commutes an
undeclared or non-scalar product, changes an inequality direction, or guesses
from notation. wasmtex supplies neutral syntax and lexical classes, Semath adds
typed mathematical meaning, and hosts consume results; dependencies never run
in the opposite direction.

The public meaning states are `established`, `partial`, `ambiguous`,
`conflicting`, and `unsupported`. A decision carries a typed reason slice:
proof, neutral uncertainty, engine limit, or demonstrated source conflict.
Decision state is not diagnostic severity. Missing engine evidence never
becomes a document warning; only exact source-linked contradictions and invalid
typed constraints do. Only deterministic results with verified side conditions
may become reviewed edits. Semath does not prove unrestricted
equivalence, infer arbitrary unstated intent, run pack code, or silently change
units and coordinate frames.

## Public boundary

Protocol 11 exposes selection, `semanticView`, definition, references, rename,
and diagnostics. `semanticView.decision` is the sole exhaustive meaning state:
established, partial, ambiguous, conflicting, or unsupported. Known decisions
carry a structured meaning; all decisions carry bounded typed reasons. There is
no summary-string or missing-evidence presentation policy in the protocol. Native, WASM,
Worker, LSP, and CorTeX consume the same result model, and raw parser trees are
not public.

Cursor ownership is one pure UTF-16 range policy over real occurrence,
structural selection, and complete-application boundaries. Exact containment
outranks trailing edges; gaps and ambiguous containers fail locally. Every
valid component position therefore resolves through the same occurrence and
entity instead of a query-specific cursor patch.

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
- Equivalent-form compilation is deterministic, capped, type-directed, and
  observable through state and guard-check budget counters.
- Domain signatures, scoped hypotheses, frontier ordering, and prose/formula
  attachment are pure bounded transformations with explicit work counters.
- Pure transformations contain normalization, inference, ranking, and view
  construction wherever practical.
- Pack declarations automatically plan positive, refusal, scope, mutation,
  macro/project, and cursor properties through an independent source grammar.
  Clean/incremental and native/WASM/Worker/LSP comparisons report the first
  divergent stage and exact field; neither oracle copies production matching.
- User interfaces explain decisions and next actions, not internal trees.
- Release WASM is built on an x86_64 Linux host, never on Apple Silicon.
