# Built-in domain packs

Semath 0.17 compiles schema-5 JSON packs into one bounded Rust semantic
runtime. Packs contain concepts, roles, laws, semantic forms, conditions,
quantities, units, activation evidence, and references. They contain no
executable code.

## Capability maturity

The checked-in [quality manifest](../fixtures/corpus-manifest.json) is the
authoritative approved support policy. Maturity is declared per capability;
the summary is only a compact view and never replaces the detailed contract.

| Pack | Summary | Evaluated capabilities | Probe capabilities | Unsupported | Laws / authored cases |
| --- | --- | --- | --- | --- | ---: |
| Circuits | evaluated | all seven | — | — | 4 / 470 |
| Classical mechanics | evaluated | all seven | — | — | 4 / 470 |
| Control systems | evaluated | all seven | — | — | 3 / 340 |
| Signals and systems | evaluated | all seven | — | — | 3 / 240 |
| Linear algebra | evaluated | all seven | — | — | 3 / 270 |
| Probability | evaluated | all seven | — | — | 2 / 180 |
| Electromagnetism | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 3 / 240 |
| Thermodynamics and heat transfer | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 3 / 240 |
| Fluid mechanics | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 3 / 240 |
| Calculus and analysis | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 1 / 90 |
| Discrete mathematics | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 2 / 180 |
| Optimization and ML | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 1 / 90 |
| Quantities and units | evaluated foundation | vocabulary, declarations, typing, refusal, project/macro, explanation | — | laws | 0 / 102 foundation |

The seven capabilities are concept vocabulary, English declarations and roles,
shape/quantity/unit typing, law recognition, diagnostic refusal,
project/macro provenance, and navigation/explanation. The dated
[maturity report](pack-maturity.md) records the evidence and remaining gaps.

Evaluated law suites require at least 30 positive and 20 refusal cases per law
across six dimensions; current diversity suites cover all seven. Probe suites
require at least 5 and 5 across three dimensions. The quantities foundation is
measured with a separate non-law suite because law count is not meaningful for
quantity, unit, dimension, and diagnostic behavior.

English prose is shared infrastructure rather than duplicated per pack. Its
foundation suite covers declarations, coordinated alignment, assumptions,
non-evidence refusal, source evidence, scope, and include order. Pack concepts
extend classification declaratively; runtime grammar never branches on pack ID.

## Authoring contract

Each pack declares `schemaVersion: 5`, stable identity and SemVer, dependencies,
concepts, and typed laws. Concepts may declare reviewed English aliases. A law
supplies canonical semantic forms and roles with a required semantic concept
and optional orthogonal quantity, shape, notation, and variadic constraints.

The Rust compiler rejects unknown fields, duplicate identities, dependency
cycles, missing or wrong-kind targets, invalid capability edges, inconsistent
dimensions, and malformed law forms. Diagnostics identify the source file and
JSON path. All laws enter the same generic unifier. A pack or law ID branch in
analysis code indicates a missing core abstraction.

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

The generated corpus is a balanced set of editable positive and refusal seeds,
not evidence of maturity by itself. The checked-in schema-5 pack files and the
[quality manifest](../fixtures/corpus-manifest.json) remain authoritative.
Repository gates run the same compiler workflow with `bun run pack:authoring`,
then conformance, foundation, and corpus evaluation.

Pack breadth does not grant edit authority. Unknown roles, incompatible types,
missing conditions, and opaque notation remain partial, ambiguous, conflicting,
or unsupported.
