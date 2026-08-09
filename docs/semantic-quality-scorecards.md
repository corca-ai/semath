# Semantic quality scorecards

Semath does not use one blended accuracy number. The v0.16 corpus reports
recall and precision for each law, plus authored refusals, role correctness,
and source-linked evidence correctness.

| Signal | Meaning | It does not mean |
| --- | --- | --- |
| Recall | authored positive cases that establish the target law | coverage of all notation in a field |
| Precision | target recognitions not present in authored negative cases | probabilistic confidence |
| Role accuracy | every expected semantic role binds the intended symbol | the formula is universally valid |
| Evidence integrity | conclusions retain nonempty source ranges | the evidence is sufficient outside its context |
| Refusal preservation | negative cases do not establish the target law | the input is mathematically false |

The checked-in corpus is independently authored and is not generated during a
test. Three architecture-proof domains contain 400 held-out cases; separate
probability and linear-algebra blind-extension cases are added only after the
generic runtime is frozen. A per-law gate prevents a broad domain from hiding a
regression in a smaller one.

Native/WASM equality, incremental affected sets, latency, memory, and package
integrity are separate gates so failures remain actionable. Threshold changes
are reviewed product decisions: correct or add labeled cases first, and never
lower a minimum solely to make CI pass.
