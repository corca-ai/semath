# Domain packs

Schema-13 JSON packs supply declarative vocabulary, typed operators and laws,
conditions, dimensions, units, and source references. They contain no executable
code. A pack's contents do not expand the [supported scope](conservative-analysis.md).

## Authority

A law's canonical relation and role constraints describe a supported operation.
They do not declare an author's symbols. The generic matcher requires source
bindings and checks each condition. Missing or conflicting evidence remains
visible; a field name or familiar notation cannot supply it.

Domain routing orders structurally compatible rules. A shared expression can
have several interpretations; routing cannot establish one or suppress an
otherwise compatible typed rule. Positive and refusal tests belong to the
operation's actual evidence requirements, not a field-wide recognition target.

## Compiler contract

Each pack declares a stable identity, SemVer, dependencies, concepts, explicit
concept bridges, and typed laws. A law provides a canonical relation, optional
representations, and role concepts with optional quantity and shape constraints.
`sourceProjection: head` binds a callable entity while retaining the authored
operator notation.

Typed operator signatures derive result constraints only when every operand
matches. Across an exact equality, the same signature may constrain the exact
expression on the other side; approximate relations and conflicting outputs do
not support this derivation.

Conditions have closed kinds, subjects bound to exact source symbols, stable
IDs, and evidence. Membership, compatible shapes, maps between spaces, operator
properties, and rank compatibility use the same source-indexed fact system.
Reviewed English phrases enter the shared declaration or assumption grammar.
There are no pack-specific prose matchers.

Concept bridges are owned by their source pack, target a direct dependency, and
connect compatible concept kinds. Ordinary parents remain local. The compiler
rejects unknown fields, duplicate identities, dependency or lineage cycles,
wrong-kind references, malformed relations, and inconsistent dimensions.

Repeated relation skeletons may use compiler archetypes with exact role-to-slot
bindings. Compilation expands them into ordinary laws; archetypes introduce no
runtime matcher. The schema-3 compiler report includes source-path diagnostics,
canonical forms, bridges, structural collisions, and domain signatures.

An established law may contribute consumed role evidence to a later formula,
bounded to two forward hops and 64 roles per target. Derived claims retain
relation parents and retract through the ordinary dependency closure.

## Pack tooling

`semath-pack` delegates compilation to Rust/WASM:

```sh
semath-pack init ./my-pack my-field
semath-pack validate ./my-pack/pack.json
semath-pack package ./my-pack/bundle.json ./my-pack/pack.json
```

`init` writes a compiler-valid example pack. Review and replace its contents
before use. `validate` checks a complete catalog; dependencies must be included.
`package` bundles sources with their compiler report.

Built-in pack discovery reads versioned JSON under `packs/`. Adding or changing
one requires focused positive, missing-evidence, contradiction, scope, and
retraction tests at the [appropriate layer](capability-test-matrix.md). Run
`bun run pack:authoring`, rebuild WASM on x86_64 Linux, then run `bun run check`
and `bun run quality`. Keep generated compiler reports out of version control.
