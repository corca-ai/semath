# Semantic quality scorecards

Semath evaluates semantic behavior with separate signals rather than a blended
accuracy number. The checked-in [quality manifest](../fixtures/corpus-manifest.json)
owns suite discovery, support tiers, coverage dimensions, transforms, and
thresholds.

| Signal | Meaning | It does not mean |
| --- | --- | --- |
| Recall | authored positive cases that recognize one target law | field-wide notation coverage |
| Precision | target recognitions absent from authored refusal cases | probabilistic confidence |
| Role accuracy | expected roles bind the intended symbols | universal validity of the formula |
| Evidence integrity | conclusions retain conditions and source ranges | sufficiency outside the source context |
| Refusal preservation | negative cases avoid recognizing the target law | proof that the input is false |
| Adversarial refusal | unknown and cross-pack collision cases recognize no law | refusal of a different, valid law in the same formula |
| Variation coverage | labeled notation, prose, role, constraint, project, macro, and mutation families | equal real-world frequency |
| Diversity cells | distinct semantic skeleton, syntax, prose, project topology, and mutation profiles | real-world prevalence |
| Metamorphic invariance | irrelevant prose, comments, and document ordering preserve outcomes | arbitrary source rewrites are safe |
| Pack-derived properties | every law receives positive, refusal, scope, mutation, macro/project, and cursor cells | production recognition agrees with itself |
| Differential equivalence | clean, incremental, native, WASM, Worker, and LSP projections agree exactly | a second semantic engine exists |
| Prose association | declared symbols receive their exact descriptions | the description is a supported domain concept |
| Assumption extraction | explicit assumptions retain subjects and evidence | hypothetical or cited properties are assumptions |
| Prose scope | declarations and assumptions obey section and include order | later or disconnected evidence is visible |

Thresholds apply to every law, so a broad suite cannot hide a weak law. The
evaluator also rejects missing or unexpected generated observations rather than
treating an empty set as success.

Corpus expectations deliberately say `recognized` or `refused`. Recognition
means that one typed relation is present in the result; it does not require the
whole cursor view to have an `established` meaning decision. Decision status is
the separate exhaustive-evidence judgment defined by the public protocol.

The current baseline has 3,106 checked-in synthetic law cases and 249
deterministic runtime variants. It includes 56 independently scored unknown,
cross-pack, and cross-field collision cases. Separate foundation suites contain
118 cases for the scientific kernel, quantities and units, and English
scientific prose. The prose suite reports association, classification,
assumption, evidence, refusal, and scope independently instead of blending them
into one recognition number.

The frozen recognition challenge is separate from those development and
diversity fixtures. Version 3 preserves the 48 independently authored v2
semantic boundaries and places every case into one reviewed document shape:
distant prose, neighboring macros, neighboring malformed input, multiple
equations, a multi-file project, or section scope. Every case declares the
expected final decision, meaning presence, and Problems policy. The scorecard
reports decision classes, source grounding, reason integrity, and problem
visibility separately from association, structure, constraints, recognition,
evidence, refusal, scope, and navigation. Exact IDs and normalized document
sources must not occur in development or foundation fixtures. Default CI
validates the schema, pure composition, and coverage matrix but does not execute
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
Pack validation also derives a deterministic bounded property plan from the
reviewed law declarations. Its oracle is the declared transformation relation,
not the production matcher. Broad generated execution and failure artifacts
remain part of the manual quality workflow; only planner integrity and a small
fixed sample run in default CI. A separate cursor plan exercises 102
native/WASM queries across eight neutral structural families and compares
semantic view, definition, references, and rename preparation at every reviewed
edge.

Native/WASM parity, full-path incremental latency and memory, package integrity,
and documentation are separate gates so failures remain actionable. The normal
performance gate covers 61 documents and the scale gate covers 501; both include
wasmtex syntax updates, adapter and Worker-host overhead, WASM analysis, cursor
queries, transfer bytes, and affected-document counts.
The manually dispatched workflow records timing on its shared runner but gates
only deterministic semantic, memory, artifact-size, transfer, and incremental
scope budgets there. Absolute timing gates remain strict on an explicitly
stable host via `bun run budget:stable`.
