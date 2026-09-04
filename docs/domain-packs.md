# Built-in domain packs

Semath compiles schema-12 JSON packs into one bounded Rust semantic
runtime. Packs contain concepts, roles, laws, canonical relations, optional
representations, conditions,
quantities, units, activation evidence, and references. They contain no
executable code.

## Capability maturity

The checked-in [quality manifest](../fixtures/corpus-manifest.json) is the
pack evaluation policy. Product support is defined by
[conservative analysis](conservative-analysis.md), independently of pack counts.
Pack maturity is declared separately for
concept vocabulary, English declarations and roles, shape/quantity/unit
typing, law recognition, diagnostic refusal, project/macro provenance, and
navigation/explanation. The dated [maturity report](pack-maturity.md) records
measured state without duplicating volatile counts here. Evaluated means only
that the declared vertical passes its capability contract; it never implies
field-wide coverage.

English prose is shared infrastructure rather than duplicated per pack. Its
foundation suite covers declarations, coordinated alignment, assumptions,
non-evidence refusal, source evidence, scope, and include order. Pack concepts
extend classification declaratively; runtime grammar never branches on pack ID.

## Authoring contract

Each pack declares `schemaVersion: 12`, stable identity and SemVer, dependencies,
concepts, explicit concept bridges, and typed laws. Concepts may declare
reviewed English aliases. A law
supplies exactly one `canonicalRelation`, optional presentation
`representations`, and roles with a required semantic concept
and optional orthogonal quantity, shape, notation, and variadic constraints.
When a role denotes the callable entity rather than its evaluated value, it may
declare `sourceProjection: head`; the public relation then retains the exact
authored operator notation without including its arguments.
Operator vocabulary may additionally declare typed operand concepts and a
result concept and shape. The generic runtime uses those signatures only when
the source establishes compatible operand types; notation alone is not proof.
Signature concepts may name either pack concepts or quantity kinds. A complete
canonical operator match can then preserve the operand roles and a declared
shared context without a law-specific runtime matcher. When that match is one
side of an exact equality, its declared result concept and constraints may
support the exact expression on the other side; explicit output shape or
quantity conflicts still refuse the role. Packs should express this reusable
evidence as an operator signature rather than adding a law-specific runtime
branch.
Each side condition has a closed kind, stable ID, validated role subjects, a
display label, and optional reviewed English `evidencePhrases`. Those phrases
compile into the shared assumption-event path; they do not create a pack
matcher. Recognition resolves subjects to bound source symbols, retains phrase
evidence, and distinguishes verified, required, conflicting, and unsupported
conditions. Activation rules carry reviewed prose `phrases` and closed
structural kinds; the removed `patterns` field is not accepted as a
compatibility alias.

Schema 11 added three compositional condition primitives without adding a second
fact system:

- `maps-between` binds an operator, domain value, and codomain value;
- `operator-property` binds one operator and one closed `operatorProperty` value;
- `rank-compatible` binds a ranked value and the extent or result it constrains.

The closed operator properties are `linear`, `bilinear`, `inner-product`,
`norm`, `adjoint`, `gradient`, `jacobian`, and `hessian`. They are semantic
requirements, not keywords or display strings. A condition's `subjects` are
resolved to exact formula bindings, and only attached source evidence can
verify it. Missing evidence leaves the condition required; notation and domain
relevance cannot promote it. Existing `domain-membership`, `shape-compatible`,
and source-indexed shape facts supply membership and compatible-extents
primitives. Law bindings now project their scalar, vector, matrix, tensor, or
function kind and any observed symbolic extents instead of flattening every
role to `expression`.

These primitives are shared only where independently named fields need the same
meaning:

| Primitive | Independent consumers |
| --- | --- |
| Typed spaces, membership, and maps | Linear algebra; differential equations; optimization |
| Rank and compatible extents | Linear algebra; statistics; numerical analysis |
| Linear and bilinear operators | Linear algebra; probability/statistics; mechanics |
| Inner products and norms | Linear algebra; optimization; numerical analysis |
| Adjoints | Linear algebra; differential equations; signal processing |
| Gradients, Jacobians, and Hessians | Calculus; optimization; numerical analysis |

Field packs remain responsible for reviewed vocabulary and laws. The generic
runtime stores these requirements, evidence, retraction edges, and work limits
without branching on a pack or law ID.

Schema 12 makes cross-pack concept lineage explicit. A `conceptBridge` is owned
by the pack that owns its source concept, targets a concept in a declared direct
dependency, and connects only equal closed concept kinds. Ordinary `parents`
remain pack-local. The compiler rejects unknown or foreign sources, undeclared
target owners, incompatible kinds, dependency cycles, concept-lineage cycles,
and external law roles whose owner is not a declared dependency. Bridges feed
the existing lineage closure; they do not create another graph or dispatch
path. The authoring report lists every bridge with its owner so reviewers can
interpret cross-pack collision evidence.

Within one document, an established law may provide its simple typed role
bindings to later formulas. Matching is source ordered, limited to 64 roles per
formula and two forward hops, and is rerun through the same compiled-law
unifier. Law-derived roles never flow backward or activate a domain hypothesis.
Roles that a later law actually consumes lower to ordinary `DerivedLaw` claims
with explicit relation parents in `ProjectSemanticIndex`; unrelated single-law
bindings are not retained. Replacement and incremental reanalysis therefore
retract the intermediate through the normal dependency closure. Missing
support, composite bindings, conflicts, and work beyond the bound remain
uncertain rather than creating a problem.

Reviewed `notation` entries on law roles may also seed a conventional candidate
when structural matching and one uniquely routed field agree. They are pack
data, not proof. The public candidate repeats the proposed bindings and carries
role-declaration requirements with the expected closed concept and shape, plus
each unverified typed condition. Authors promote the proposal only by adding
ordinary visible declarations or a reviewed source edit; packs cannot silently
declare a document's symbols by convention.

For relation skeletons repeated across independent laws and fields, a law may
replace `canonicalRelation` with one reviewed `archetype` and an exact
role-to-slot binding. Slots must bind every law role once. The Rust authoring
compiler expands the specialization to the same `PackLaw` IR before runtime;
there is no archetype runtime, fallback, or second matcher. The compiler report
shows matching and adopted laws, and duplicate or incomplete expansions fail
validation.

The Rust compiler rejects unknown fields, duplicate identities, dependency
cycles, missing or wrong-kind targets, invalid capability edges, inconsistent
dimensions, and malformed law forms. Diagnostics identify the source file and
JSON path. All laws enter the same generic unifier. A pack or law ID branch in
analysis code indicates a missing core abstraction.

The authoring report schema is version 3. Alongside diagnostics and canonical
forms it exposes catalog-derived domain signatures, explicit bridge ownership,
and a deterministic cross-pack collision atlas. Authors review ambiguity and
refusal ownership from this report; generated reports remain build artifacts
rather than checked-in documentation.

Every new law must have an owning corpus suite with positive, refusal, role,
notation, constraint, and project-context evidence appropriate to its maturity.
Foundation capabilities require a suite that observes their actual public
outputs.

## Authoring workflow

`semath-pack` is the public authoring boundary. It delegates pack compilation
to Rust/WASM and uses TypeScript only for pure corpus planning, scorecard diffs,
and presentation. A minimal workflow is:

```sh
semath-pack init ./my-pack my-field
# Replace the sample concepts, law, reference, and reviewed corpus cells.
semath-pack validate ./my-pack/pack.json
semath-pack scaffold ./my-pack/pack.json ./my-pack/reviewed
semath-pack package ./my-pack/bundle.json ./my-pack/pack.json
```

To exercise a pack in the engine, add its versioned JSON under `packs/`, add
its reviewed suites to the quality manifest, and rebuild. Pack discovery is
automatic; no Rust registry or recognizer branch is edited. Then use
`semath-pack score <manifest> <pack-id>` for a focused scorecard and
`semath-pack explain <manifest> <pack-id> <case-id>` for one decision. Compare
reviewed results with `semath-pack compare <baseline> <candidate>`.

The generated corpus is a balanced set of positive and refusal observations
materialized from compact editable seeds, not independently authored evidence
or evidence of maturity by itself. The checked-in schema-12 pack files and the
[quality manifest](../fixtures/corpus-manifest.json) remain authoritative.
Repository gates run the same compiler workflow with `bun run pack:authoring`,
then conformance followed by the manual foundation and corpus evaluation.

Pack breadth does not grant edit authority. Unknown roles, incompatible types,
missing conditions, and opaque notation remain partial, ambiguous, conflicting,
or unsupported.
