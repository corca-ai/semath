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
| Variation coverage | labeled notation, prose, role, constraint, project, macro, and mutation families | equal real-world frequency |
| Metamorphic invariance | irrelevant prose, comments, and document ordering preserve outcomes | arbitrary source rewrites are safe |

Thresholds apply to every law, so a broad suite cannot hide a weak law. The
evaluator also rejects missing or unexpected generated observations rather than
treating an empty set as success.

The 0.17 baseline has 420 checked-in synthetic cases, 40 deterministic runtime
variants, 372 variation tags, and 59 refusal categories. Evaluated laws require
100% role, evidence, and refusal preservation, at least 99% precision, and at
least 95% recall. This baseline is release evidence, not a completeness claim or
live production telemetry.

Run the concise gate or write a stable machine-readable artifact with:

```sh
bun run corpus
bun run scorecard
```

The latter writes `.artifacts/semantic-scorecard.json`. Generated scorecards and
metamorphic fixtures are not version-controlled. Fix labels or behavior before
changing a threshold; never lower one solely to make CI pass.

Native/WASM parity, incremental latency and memory, package integrity, and
documentation are separate gates so failures remain actionable.
