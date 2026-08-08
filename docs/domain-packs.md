# Built-in domain packs

Semath v0.12 ships one schema-2 catalog consumed by domain activation, formula
recognition, completion, and rewrite orchestration. `semath/packs` exposes the
same metadata to TypeScript consumers; `semath-pack` validates the built-ins or
one or more JSON paths without CorTeX.

```sh
bunx semath-pack
bunx semath-pack ./my-pack.json
```

## Safety and maturity

Pack breadth does not grant edit authority. Every pattern declares one maximum
maturity:

- `recognition` identifies and explains a calibrated surface form only;
- `completion` additionally requires an explicitly typed target and compatible,
  visible inputs before producing a review-required insertion;
- `diagnostic` is reserved for contradictions backed by strong evidence;
- `rewrite` additionally requires exact source text and explicit side-condition
  refinements before producing a review-required replacement.

Weak vocabulary or notation activation contributes domain context only. It
cannot create a definition, warning, completion, or rewrite. Runtime plugin code
and unknown matcher or constraint primitives are rejected.

## Catalog matrix

The JSON catalog is the machine-readable topic/capability matrix. It currently
contains 68 formula entries, including 59 recognition-only forms and nine
action-capable typed forms.

| Pack | Topics represented | Entries | Action-capable subset |
| --- | --- | ---: | --- |
| Linear algebra | products, transpose, forms, invariants, norms, systems, eigenvalues, factorizations, least squares | 15 | five typed completions |
| Probability and statistics | events, conditional probability, moments, distributions, information theory, estimators | 15 | four typed completions; two guarded rewrites |
| Calculus and analysis | ordinary/partial derivatives, gradient/Hessian/Laplacian, integrals, limits, sums/products, divergence/curl | 12 | recognition only |
| Optimization and ML | min/max, constraints, Lagrangians, updates, least squares, regularization, sigmoid/softmax, empirical risk, cross-entropy | 12 | recognition only |
| Discrete mathematics | sets, quantifiers, implication, combinatorics, recurrences, graph degree/Laplacian | 14 | recognition only |

Every entry is exercised through at least five supported surfaces and five
structural refusals. Thirteen focused collision documents cover every pair of
packs plus within-pack binder and estimator ambiguities. The nine action-capable
patterns have a separate exact-result corpus; recognition-only entries must
produce no edits. Native and release WASM must agree on results, ranges,
bindings, conditions, ranks, and refusals.

Deliberately unsupported in v0.12 are equivalence execution, theorem-assumption
proof, convergence/convexity inference, arbitrary user regex loading, physical
units, and automatic logical rewrites. Those forms remain recognition-only or
unknown until stronger semantics and calibration exist.

## Schema authoring contract

A pack contains metadata, activation literals, role/operator vocabulary,
formula patterns, optional rewrites, and references. Each entry owns a topic,
user-facing description and stable description key, maturity, source
references, and a named bounded primitive. The pure loader validates:

- schema and SemVer compatibility;
- unique kebab-case IDs and reference links;
- known matcher, constraint, and side-condition primitives;
- bounded non-empty regexes whose capture count matches parameters;
- parameter, dimension, refinement, and template placeholders;
- one human condition description per declared side condition;
- rewrite source maturity, parameters, and explicit refinements;
- the rule that recognition-only entries cannot contain edit templates.

Adding a built-in pack requires registering its JSON source once in the catalog;
domain, pattern, completion, and rewrite orchestration contain no pack-ID branch.
The schema is intentionally for trusted built-ins: it is not a runtime plugin
mechanism or an unbounded rule language.
