# Built-in domain packs

Semath compiles schema-10 JSON packs into one bounded Rust semantic
runtime. Packs contain concepts, roles, laws, canonical relations, optional
representations, conditions,
quantities, units, activation evidence, and references. They contain no
executable code.

## Capability maturity

The checked-in [quality manifest](../fixtures/corpus-manifest.json) is the
authoritative approved support policy. Maturity is declared separately for
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

Each pack declares `schemaVersion: 10`, stable identity and SemVer, dependencies,
concepts, and typed laws. Concepts may declare reviewed English aliases. A law
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
shared context without a law-specific runtime matcher.
Each side condition has a closed kind, stable ID, validated role subjects, a
display label, and optional reviewed English `evidencePhrases`. Those phrases
compile into the shared assumption-event path; they do not create a pack
matcher. Recognition resolves subjects to bound source symbols, retains phrase
evidence, and distinguishes verified, required, conflicting, and unsupported
conditions. Activation rules carry reviewed prose `phrases` and closed
structural kinds; the removed `patterns` field is not accepted as a
compatibility alias.

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
forms it exposes catalog-derived domain signatures and a deterministic
cross-pack collision atlas. Authors review ambiguity and refusal ownership from
this report; generated reports remain build artifacts rather than checked-in
documentation.

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
or evidence of maturity by itself. The checked-in schema-10 pack files and the
[quality manifest](../fixtures/corpus-manifest.json) remain authoritative.
Repository gates run the same compiler workflow with `bun run pack:authoring`,
then conformance followed by the manual foundation and corpus evaluation.

Pack breadth does not grant edit authority. Unknown roles, incompatible types,
missing conditions, and opaque notation remain partial, ambiguous, conflicting,
or unsupported.
