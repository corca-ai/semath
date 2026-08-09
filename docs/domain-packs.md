# Built-in domain packs

Semath v0.15 ships one schema-3 catalog used by semantic concepts, relations,
domain activation, formula recognition, completion, diagnostics, and rewrites.
`semath/packs` exposes the same immutable metadata and validators to TypeScript;
`semath-pack` validates JSON without loading an editor.

```sh
bunx semath-pack
bunx semath-pack ./my-pack.json
```

## Safety and maturity

Pack breadth does not grant edit authority. `recognition` only identifies a
calibrated surface. `completion` and `rewrite` may produce revision-checked,
review-required proposals when their explicit constraints hold. `diagnostic`
is reserved for evidence-backed contradictions. Weak vocabulary priors cannot
create definitions, warnings, or edits.

## Catalog

The five established mathematical packs retain their guarded edit authority
while adding recognition depth. The engineering pilots now cover coherent
families of laws rather than one demonstration each.

| Pack | Responsibility | Edit authority |
| --- | --- | --- |
| Linear algebra | shapes, products, forms, decompositions | calibrated completions |
| Probability/statistics | events, distributions, estimators | calibrated completions and guarded rewrites |
| Calculus/analysis | derivatives, integrals, limits, vector calculus | recognition only |
| Optimization/ML | objectives, constraints, updates, losses | recognition only |
| Discrete mathematics | sets, logic, combinatorics, recurrences, graphs | recognition only |
| Quantities/units | exact dimensions, SI unit vocabulary, propagation | no formula edits |
| Classical mechanics | force, momentum, energy, spring/damping, power, circular motion | recognition only |
| Circuits | Ohm/Kirchhoff laws, capacitor/inductor relations and energy, power | recognition only |
| Control systems | continuous/discrete state space, feedback, closed loop, Lyapunov equations | recognition only |

The checked-in synthetic corpus has at least 50 independently authored cases
per field, split into positive surfaces, hard refusals, and unsupported coverage
targets. v0.15 promotes 41 previously unsupported targets while preserving 129
untouched formula holdouts. Exact vertical fixtures additionally assert quantity
facts, dimensions, relation roles, cross-pack collisions, diagnostics, and
native/WASM parity.

The catalog is intentionally conservative: a familiar surface is not enough
when a law requires quantity or shape evidence. Adding patterns cannot weaken
the checked refusal and holdout budgets. See
[`semantic-quality-scorecards.md`](semantic-quality-scorecards.md).

## Schema-3 authoring contract

A pack declares a stable namespace, kind, SemVer, dependencies, provided and
required capabilities, concepts, optional quantity kinds and units, laws,
patterns, rewrites, and references. The canonical JSON Schema is
[`schemas/domain-pack-v3.schema.json`](../schemas/domain-pack-v3.schema.json).
The Rust and TypeScript validators additionally enforce catalog-wide rules that
JSON Schema alone cannot express:

- dependencies exist, match the requested major version, and are acyclic;
- required capabilities are supplied by declared dependencies;
- external concepts and units come only from the pack's dependency closure;
- default units exist and have the quantity kind's exact dimension;
- law roles and pattern concept constraints resolve to known concepts;
- bounded matcher primitives, reference links, maturity, and edit templates are coherent.

Arbitrary runtime code, unbounded regex execution, implicit unit conversion,
theorem proving, and automatic scientific rewrites remain outside this contract.
New fields should normally be added as data plus corpus expectations; pack-ID
branches in orchestration are a design failure.
