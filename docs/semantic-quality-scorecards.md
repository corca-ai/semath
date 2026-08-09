# Semantic quality scorecards

Semath does not use one blended “accuracy” number as a release decision. The
v0.15 gates report exact behavior by field, domain, topic, and capability, then
compare the underlying integer counters with the versioned budgets in
`fixtures/v0.15/semantic-quality-budgets.json`.

## Signals

| Signal | Meaning | It does not mean |
| --- | --- | --- |
| Case accuracy | Cases whose complete result equals the checked expectation | General accuracy on unseen papers |
| Recall | Expected labeled items that were returned | Coverage of all notation in a field |
| Precision | Returned items that belong to the checked expectation | A probabilistic confidence estimate |
| Refusal preservation | Near misses that retain their exact, usually empty, result | Unsupported input is mathematically invalid |
| Holdout preservation | Unpromoted coverage targets remain unchanged | The target should never be supported |
| Unexpected items | Extra patterns or definitions, including cross-pack collisions | Every false positive possible in real documents |

Formula and English-prose corpora are independently authored and de-duplicated.
When a coverage target is promoted, its expectation changes but the remaining
holdouts stay untouched. Per-domain minimums prevent a broad pack from hiding a
regression in a smaller one.

Golden protocol fixtures separately enforce zero known false definitions,
references, diagnostics, completions, rewrites, and renames. The same run checks
native/WASM equality. Latency, retained memory, response size, cold start, and
WASM size remain separate performance budgets because combining them with
semantic correctness would make failures less actionable.

## Changing a budget

A budget change is a reviewed product decision, not an automatic consequence of
a failing test. Add or correct labeled cases first, explain any threshold change
in the pull request, and never reduce a minimum merely to make CI pass. Score
classification, aggregation, and comparison are pure functions; process launch,
file reads, and console reporting stay in the runner scripts.
