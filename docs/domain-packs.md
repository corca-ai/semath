# Built-in domain packs

Semath 0.17 compiles schema-4 JSON packs into one bounded Rust semantic
runtime. Packs contain concepts, roles, laws, semantic forms, conditions,
quantities, units, activation evidence, and references. They contain no
executable code.

## Support tiers

The checked-in [quality manifest](../fixtures/corpus-manifest.json) is the
authoritative support policy. A large vocabulary is not presented as evaluated
law recognition.

| Pack | Tier | Laws covered | Authored cases |
| --- | --- | ---: | ---: |
| Circuits | evaluated | 3/3 | 390 |
| Classical mechanics | evaluated | 3/3 | 390 |
| Control systems | evaluated | 2/2 | 260 |
| Linear algebra | probe | 1/1 | 90 |
| Probability | probe | 1/1 | 90 |
| Calculus and analysis | vocabulary-only | 0/0 | 0 |
| Discrete mathematics | vocabulary-only | 0/0 | 0 |
| Optimization and ML | vocabulary-only | 0/0 | 0 |
| Quantities and units | vocabulary-only | 0/0 | 0 |

Evaluated packs require at least 30 positive and 20 refusal cases per law and at
least six coverage dimensions; the current suites require all seven. Probe
packs require 5 and 5 across three dimensions. Vocabulary-only packs may
provide concepts and activation evidence but cannot silently add untested laws.

## Authoring contract

Each pack declares `schemaVersion: 4`, stable identity and SemVer, dependencies,
concepts, and typed laws. A law supplies canonical semantic forms and roles
with optional concept, shape, quantity, and variadic constraints.

The Rust compiler rejects unknown fields, dependency cycles, missing concepts,
invalid capability edges, inconsistent dimensions, and malformed law forms.
All laws enter the same generic unifier. A pack or law ID branch in analysis
code indicates a missing core abstraction.

Every new law must have an owning corpus suite with positive, refusal, role,
notation, constraint, and project-context evidence appropriate to its tier.
Run:

```sh
bun run pack:conformance
bun run corpus
```

Pack breadth does not grant edit authority. Unknown roles, incompatible types,
missing conditions, and opaque notation must remain partial, ambiguous,
conflicting, or unsupported.
