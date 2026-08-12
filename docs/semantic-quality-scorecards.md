# Semantic quality scorecards

Semath evaluates semantic behavior with separate signals rather than a blended
accuracy number. The checked-in [quality manifest](../fixtures/corpus-manifest.json)
owns suite discovery, support tiers, coverage dimensions, transforms, and
thresholds.

| Signal | Meaning | It does not mean |
| --- | --- | --- |
| Recall | reviewed positive cases that recognize one target law | field-wide notation coverage |
| Precision | target recognitions absent from reviewed refusal cases | probabilistic confidence |
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

Current measured counts live only in the dated
[pack maturity report](pack-maturity.md). This guide defines signals and policy,
not a second baseline that can silently go stale.

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

The recognition-frontier fixture is a smaller stage-specific diagnostic gate.
Its 32 frozen cases span notation surfaces, probability/statistics/ML,
continuum engineering, fields and waves, optimization/control/signals,
discourse scope, project and macro lifecycle, and refusal behavior. Each case
records both the pre-change observation and the intended decision, relation,
and first unresolved stage. `bun run frontier:fixture` checks only schema and
pure scoring in ordinary CI; `bun run frontier` executes the engine as a manual
release gate. A missing type or sign convention must remain partial even when
the relation is known, and unsupported input must not be promoted to a user
problem.

The independent scoped-domain challenge adds 30 document-shaped cases across
neutral document fields, section scope, mixed domains, non-evidence,
formula-before/after attachment, ambiguity, conflict, and retraction. It
declares ordered domain tiers, final decision state, and Problems policy. Six
additional cases exercise every current cross-law collision component with
independently typed formula contexts; the fixture records why unsupported or
conflicting cells belong to refusal or source-conflict policy rather than
domain ranking. Normalized challenge documents
must not occur in development or generated fixtures. `bun run domain:fixture`
is a fast schema/leakage gate; `bun run domain:challenge` is a manual release
gate.

The frozen semantic-continuity holdout adds 48 independently authored cases in
six equal families: lifetime and shadowing, notation identity, discourse flow,
canonical structure, typed propagation, and safety or retraction. Each case
records the pre-v0.26 decision and Problems count separately from its reviewed
target. The scorer weights false establishment, false conflict, and identity
leakage above missed coverage and reports every family independently.
Normalized holdout documents must not occur in development or generated
fixtures. `bun run continuity:fixture` checks schema, diversity, leakage, and
the pure scorer in ordinary CI; `bun run continuity` preserves the original
frozen-target report. `bun run continuity:release` is the deliberate release
gate. It applies only the reviewed decision transitions listed in release code
and still scores every other expectation from the immutable fixture.

The v0.27 authored scientific tranche adds 96 editable development scenarios
and 48 separately authored frozen holdout scenarios. Its 212 probes observe
meaning, definition, references, rename preparation, rename, and Problems at
reviewed source positions across all 61 current laws. Exactly eight holdout
scenarios cover each document-reasoning family: scope-bound comparisons,
derivation chains, guarded conditions, discourse references, collisions or
unsupported input, and edit lifecycles. Every source and expectation was
written engine-blind, independently critiqued, fully reviewed and corrected by
the main agent, then sealed before its first engine run. Alpha-renaming-aware
wasmtex CST fingerprints and math-masked five-word prose shingles guard split
lineage without introducing another parser. `bun run authored:fixture` performs
only schema, provenance, digest, coverage, anchor, and leakage checks in normal
CI. `bun run authored:baseline` and `bun run authored` execute the six public
query surfaces only as deliberate manual evidence gates.

The v0.28 release adds a new 48-scenario blind tranche rather than reusing the
historical holdout as a tuning target. It has exactly eight scenarios in each
document-reasoning family and covers every decision class. Isolated Codex
subagents author its source and expectations without engine execution;
independent subagents critique them; then the main agent reads every source and
expectation, applies corrections, freezes the task card and review digests, and
seals both the authored fixture and its release envelope. Author, critic, and
main-review identities must be distinct, and the validator rejects source
reuse or suspicious math/prose similarity with any checked-in fixture.

The blind path is always explicit: set `SEMATH_FRESH_BLIND_FIXTURE` and
`SEMATH_FRESH_BLIND_RECEIPT`, then run `bun run release:semantic` on the separate
x86_64 Linux release host. All pre-blind checks—including the editable
development tranche, final historical regression suites, stable performance
budget, WASM build and checksum, package smoke test, and docs lint—must pass
first. The
runner exclusively creates the receipt immediately before the first engine
query, refuses to reuse a receipt path, evaluates all six public query surfaces,
and compares clean rebuilds with incremental snapshot transitions. The receipt
retains fixture and artifact seals, exact Semath and wasmtex revisions, score,
first-loss atlas, lifecycle result, and separate safety counts. Missed coverage
is reported honestly; false establishment, false conflict, or unsafe navigation
and edit fail the release. After that single run, the tranche becomes historical
evidence and must not guide implementation changes.

The editable development tranche is a reviewed coverage frontier, not a
115/115 conformance suite. Its release gate permits documented misses but
rejects any false establishment, false conflict, navigation or identity risk,
fewer than 50 passing probes, or risk above 130. Tighten this dated baseline
when coverage improves; do not weaken it to admit a regression.

The 2026-08-13 v0.28 pre-blind evaluation passes 50 of 115 editable development
probes (risk 130, no false establishment, false conflict, or navigation risk).
The immutable semantic-continuity report is 22 of 48 with raw risk 202; fifteen
reviewed `partial` to `established` transitions now have exact source-backed
definitions or relations. Applying only those explicit adjudications produces
the release score 37 of 48 with risk 22 and no unsafe risk. The remaining eleven
cases stay visible as coverage work.

The now-exposed historical authored holdout remains 6 of 97 with risk 720.
These deliberately difficult documents are a first-loss map, not a release
pass-rate target. Its two raw false-establishment counts are exact, reviewed
frozen-contract disagreements. One lifecycle expects prose in a disconnected
project component to retract a relation in another component; Semath preserves
the dependency boundary. The other expects `partial` for prose that directly
asserts an overlap as \(A\cap B\); the current source-grounded entity decision is
`established`. The release gate permits only these named, proof-grounded cases
and rejects substitution by any new false establishment. Current historical
first losses are 23 attachment, 42 identity or scope, 10 typed-fact, 12
pack-unification, and four decision cases. See
[Pack maturity](pack-maturity.md) for interpretation by capability.

Evaluated laws require
100% role, evidence, and refusal preservation, at least 99% precision, and at
least 95% recall. This baseline is release evidence, not a completeness claim or
live production telemetry. It contains no imported real-world corpus, so it must
not be used to claim field completeness or real-world frequency.

Semantic corpus execution is a deliberate manual/release gate, not a default
pull-request check. Default CI still parses schemas, verifies the compact
materialization ledger and generator determinism, tests the pure scorers, and
enforces pack conformance. This keeps
ordinary feedback fast without presenting an expensive synthetic evaluation as
continuous production evidence.

Run the complete manual gate, a focused evaluation, or write a stable
machine-readable artifact with:

```sh
bun run quality
bun run challenge
bun run challenge:report
mkdir -p .artifacts && bun run frontier:baseline
bun run frontier
mkdir -p .artifacts && bun run domain:baseline
bun run domain:challenge
mkdir -p .artifacts && bun run continuity:baseline
bun run continuity
bun run continuity:release
mkdir -p .artifacts && bun run authored:baseline
bun run authored
bun run authored:development
bun run authored:development:release
bun run authored:historical:release
bun run corpus
bun run corpus:generate:check
bun run foundation
bun run foundation:generate:check
bun run scorecard
```

The fresh blind commands are intentionally absent from the generic examples:
they require an independently commissioned, sealed v0.28 fixture and a new
receipt path. `bun run fresh-blind:validate` checks that explicit fixture without
executing Semath. `bun run fresh-blind:run` is the one-shot evidence boundary and
should normally be reached only through `bun run release:semantic`.

`bun run challenge:report` writes `.artifacts/recognition-challenge.json`, and
`bun run scorecard` writes `.artifacts/semantic-scorecard.json`. The same
complete gate is available through the manually dispatched `semantic-quality`
GitHub workflow. Generated scorecards and metamorphic fixtures are not
version-controlled. Fix labels or behavior before changing a threshold; never
lower one solely to make a gate pass.

The checked-in generation specification separates declaration, prose,
presentation, project, macro, constraint, mutation, and cursor batches. The
5,406 deterministic cases are materialized in memory for evaluation rather
than checked in as expanded JSON. The compact
[materialization ledger](../fixtures/corpus-materialization.json) freezes suite
counts and canonical digests and rejects tracked expansions. Pure generation
and integrity checks reject nondeterminism, normalized duplicates,
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

## v0.27 corpus compaction evidence

On 2026-08-12, baseline commit
`f8431e8ef967471a913fd1de0e53c289658d39c1` stored 33 corpus files containing
251,298 lines and 9,545,100 bytes. The compact representation keeps five
reviewable fixture files containing 14,878 lines and 560,138 bytes, a reduction
of 94.1% in both tracked lines and bytes. It still materializes all 5,406
deterministic cases, and the 420 retained fixture cases keep the scored total at
5,826. The materialization ledger records the canonical case count, byte count,
and SHA-256 digest of every removed expansion.

Before removing the expanded files, each materialized suite was compared
byte-for-byte with its tracked baseline output. Full corpus execution remains a
manual release measurement because it is expensive and does not provide useful
pull-request latency. A same-machine development run after compaction took
881.04 seconds and reached 3,981,983,744 bytes of peak resident memory. It
reproduced the baseline's four existing precision/refusal failures exactly;
compaction changed neither inputs nor observations. After the v0.27 semantic
refinements, the final manual run passes all thresholds for all 5,826 cases and
all 453 metamorphic observations: every one of the 61 evaluated laws has 100%
recall, precision, role accuracy, evidence integrity, and refusal preservation.
These local measurements are diagnostic evidence, not the stable x86_64 release
budget measurement.

Native/WASM parity, full-path incremental latency and memory, package integrity,
and documentation are separate gates so failures remain actionable. The normal
performance gate covers 61 documents and the scale gate covers 501; both include
wasmtex syntax updates, adapter and Worker-host overhead, WASM analysis, cursor
queries, transfer bytes, and affected-document counts.
The manually dispatched workflow records timing on its shared runner but gates
only deterministic semantic, memory, artifact-size, transfer, and incremental
scope budgets there. Absolute timing gates remain strict on an explicitly
stable host via `bun run budget:stable`.
