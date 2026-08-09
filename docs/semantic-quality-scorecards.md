# Semantic quality scorecards

Semath evaluates semantic behavior with separate signals rather than a blended
accuracy number. The checked-in [quality manifest](../fixtures/corpus-manifest.json)
owns suite discovery, support tiers, coverage dimensions, transforms, and
thresholds.

| Signal | Meaning | It does not mean |
| --- | --- | --- |
| Recall | authored positive cases that establish one target law | field-wide notation coverage |
| Precision | target recognitions absent from authored refusal cases | probabilistic confidence |
| Role accuracy | expected roles bind the intended symbols | universal validity of the formula |
| Evidence integrity | conclusions retain conditions and source ranges | sufficiency outside the source context |
| Refusal preservation | negative cases avoid establishing the target law | proof that the input is false |
| Adversarial refusal | unknown and cross-pack collision cases establish no law | refusal of a different, valid law in the same formula |
| Variation coverage | labeled notation, prose, role, constraint, project, macro, and mutation families | equal real-world frequency |
| Diversity cells | distinct semantic skeleton, syntax, prose, project topology, and mutation profiles | real-world prevalence |
| Metamorphic invariance | irrelevant prose, comments, and document ordering preserve outcomes | arbitrary source rewrites are safe |

Thresholds apply to every law, so a broad suite cannot hide a weak law. The
evaluator also rejects missing or unexpected generated observations rather than
treating an empty set as success.

The current baseline has 1,890 checked-in synthetic law cases and 157
deterministic runtime variants. It includes 40 independently scored unknown and
cross-pack collision cases. A separate 46-case foundation corpus evaluates
quantity, unit, dimension, and diagnostic behavior without inventing laws.
Evaluated laws require
100% role, evidence, and refusal preservation, at least 99% precision, and at
least 95% recall. This baseline is release evidence, not a completeness claim or
live production telemetry. It contains no imported real-world corpus, so it must
not be used to claim field completeness or real-world frequency.

Run the concise gate or write a stable machine-readable artifact with:

```sh
bun run corpus
bun run corpus:generate:check
bun run foundation
bun run foundation:generate:check
bun run scorecard
```

The latter writes `.artifacts/semantic-scorecard.json`. Generated scorecards and
metamorphic fixtures are not version-controlled. Fix labels or behavior before
changing a threshold; never lower one solely to make CI pass.

The checked-in generation specification separates declaration, prose,
presentation, project, macro, constraint, mutation, and cursor batches. Pure
generation and integrity checks reject stale output, normalized duplicates,
duplicate semantic/syntax/prose/tag profiles, malformed delimiters, invalid
environment nesting, ambiguous cursors, and leaked fixture identities.

Native/WASM parity, incremental latency and memory, package integrity, and
documentation are separate gates so failures remain actionable.
