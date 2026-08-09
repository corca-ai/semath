# Built-in domain packs

Semath 0.17 compiles schema-4 JSON packs into one bounded Rust semantic
runtime. Packs contain concepts, roles, laws, semantic forms, conditions,
quantities, units, activation evidence, and references. They contain no
executable code.

## Capability maturity

The checked-in [quality manifest](../fixtures/corpus-manifest.json) is the
authoritative approved support policy. Maturity is declared per capability;
the summary is only a compact view and never replaces the detailed contract.

| Pack | Summary | Evaluated capabilities | Probe capabilities | Unsupported | Laws / authored cases |
| --- | --- | --- | --- | --- | ---: |
| Circuits | evaluated | all seven | — | — | 3 / 390 |
| Classical mechanics | evaluated | all seven | — | — | 3 / 390 |
| Control systems | evaluated | all seven | — | — | 2 / 260 |
| Linear algebra | evaluated | all seven | — | — | 3 / 270 |
| Probability | evaluated | all seven | — | — | 2 / 180 |
| Calculus and analysis | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 1 / 90 |
| Discrete mathematics | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 2 / 180 |
| Optimization and ML | mixed | vocabulary | declarations, typing, laws, refusal, project/macro, explanation | — | 1 / 90 |
| Quantities and units | evaluated foundation | vocabulary, declarations, typing, refusal, explanation | — | laws, project/macro | 0 / 46 foundation |

The seven capabilities are concept vocabulary, English declarations and roles,
shape/quantity/unit typing, law recognition, diagnostic refusal,
project/macro provenance, and navigation/explanation. The dated
[maturity report](pack-maturity.md) records the evidence and remaining gaps.

Evaluated law suites require at least 30 positive and 20 refusal cases per law
across six dimensions; current diversity suites cover all seven. Probe suites
require at least 5 and 5 across three dimensions. The quantities foundation is
measured with a separate non-law suite because law count is not meaningful for
quantity, unit, dimension, and diagnostic behavior.

## Authoring contract

Each pack declares `schemaVersion: 4`, stable identity and SemVer, dependencies,
concepts, and typed laws. A law supplies canonical semantic forms and roles
with optional concept, shape, quantity, notation, and variadic constraints.

The Rust compiler rejects unknown fields, dependency cycles, missing concepts,
invalid capability edges, inconsistent dimensions, and malformed law forms.
All laws enter the same generic unifier. A pack or law ID branch in analysis
code indicates a missing core abstraction.

Every new law must have an owning corpus suite with positive, refusal, role,
notation, constraint, and project-context evidence appropriate to its maturity.
Foundation capabilities require a suite that observes their actual public
outputs. Run:

```sh
bun run pack:conformance
bun run foundation
bun run corpus
```

Pack breadth does not grant edit authority. Unknown roles, incompatible types,
missing conditions, and opaque notation remain partial, ambiguous, conflicting,
or unsupported.
