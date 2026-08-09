# Built-in domain packs

Semath 0.16 ships schema-4 packs compiled into the Rust semantic runtime.
Packs contain typed concepts, roles, laws, semantic forms, conditions,
quantities, units, activation evidence, and references. They do not contain
executable code.

## Catalog

| Pack | Main knowledge |
| --- | --- |
| Linear algebra | shapes, operators, matrix/vector relations |
| Probability | events, distributions, estimators, probability relations |
| Calculus and analysis | derivatives, integrals, limits, vector calculus |
| Optimization and ML | objectives, constraints, updates, losses |
| Discrete mathematics | sets, logic, combinatorics, recurrences, graphs |
| Quantities and units | exact dimensions, SI vocabulary, propagation |
| Classical mechanics | force, energy, momentum, power, motion |
| Circuits | Kirchhoff and constitutive laws, energy, power |
| Control systems | state space, feedback, and Lyapunov relations |

## Authoring contract

Each JSON file must declare `schemaVersion: 4`, a stable namespace and SemVer,
dependencies, concepts, and typed laws. A law supplies canonical semantic forms
and roles with optional concept, shape, quantity, and variadic constraints.

The Rust compiler rejects unknown fields, dependency cycles, missing concepts,
invalid capability edges, inconsistent dimensions, and malformed law forms.
All accepted laws enter the same bounded generic unifier. A pack ID or law ID
branch in analysis code indicates a missing core abstraction and must be
removed.

Pack changes require independently authored positive, negative, ambiguous, and
cross-file cases. `bun run corpus:v0.16` checks recognition, role binding,
source-linked evidence, and safe refusal. The blind-extension fixtures prove
that probability and linear-algebra laws can be added through pack data without
adding recognizer code.

Pack breadth does not grant automatic edit authority. Unknown roles,
incompatible types, missing conditions, and opaque notation produce partial,
ambiguous, conflicting, or unsupported results.
