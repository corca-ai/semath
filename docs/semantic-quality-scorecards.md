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
| Prose association | declared symbols receive their exact descriptions | the description is a supported domain concept |
| Assumption extraction | explicit assumptions retain subjects and evidence | hypothetical or cited properties are assumptions |
| Prose scope | declarations and assumptions obey section and include order | later or disconnected evidence is visible |

Thresholds apply to every law, so a broad suite cannot hide a weak law. The
evaluator also rejects missing or unexpected generated observations rather than
treating an empty set as success.

The current baseline has 3,050 checked-in synthetic law cases and 249
deterministic runtime variants. It includes 56 independently scored unknown,
cross-pack, and cross-field collision cases. Separate foundation suites contain
102 cases for the scientific kernel, quantities and units, and English
scientific prose. The prose suite reports association, classification,
assumption, evidence, refusal, and scope independently instead of blending them
into one recognition number.

The frozen recognition challenge is separate from those development and
diversity fixtures. Its cases are manually authored and grouped by the layer
that owns a failure: syntax, binding, constraint, pack, resolution, or
presentation. Association, structure, constraint, recognition, evidence,
refusal, scope, and navigation remain separate metrics. Default CI validates
the challenge schema and coverage matrix through pure tests but does not execute
the engine over the holdout.
Evaluated laws require
100% role, evidence, and refusal preservation, at least 99% precision, and at
least 95% recall. This baseline is release evidence, not a completeness claim or
live production telemetry. It contains no imported real-world corpus, so it must
not be used to claim field completeness or real-world frequency.

Semantic corpus execution is a deliberate manual/release gate, not a default
pull-request check. Default CI still parses schemas, verifies generated fixture
integrity, tests the pure scorers, and enforces pack conformance. This keeps
ordinary feedback fast without presenting an expensive synthetic evaluation as
continuous production evidence.

Run the complete manual gate, a focused evaluation, or write a stable
machine-readable artifact with:

```sh
bun run quality
bun run challenge
bun run challenge:report
bun run corpus
bun run corpus:generate:check
bun run foundation
bun run foundation:generate:check
bun run scorecard
```

`bun run challenge:report` writes `.artifacts/recognition-challenge.json`, and
`bun run scorecard` writes `.artifacts/semantic-scorecard.json`. The same
complete gate is available through the manually dispatched `semantic-quality`
GitHub workflow. Generated scorecards and metamorphic fixtures are not
version-controlled. Fix labels or behavior before changing a threshold; never
lower one solely to make a gate pass.

The checked-in generation specification separates declaration, prose,
presentation, project, macro, constraint, mutation, and cursor batches. Pure
generation and integrity checks reject stale output, normalized duplicates,
duplicate semantic/syntax/prose/tag profiles, malformed delimiters, invalid
environment nesting, ambiguous cursors, and leaked fixture identities.

Native/WASM parity, full-path incremental latency and memory, package integrity,
and documentation are separate gates so failures remain actionable. The normal
performance gate covers 61 documents and the scale gate covers 501; both include
wasmtex syntax updates, adapter and Worker-host overhead, WASM analysis, cursor
queries, transfer bytes, and affected-document counts.
