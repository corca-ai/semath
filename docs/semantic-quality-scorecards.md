# Historical broad-STEM quality scorecards

This document preserves the broad-recognition evaluation policy and its
historical evidence. Its field-wide coverage and one-shot commissioning gates
are superseded for product releases by [conservative analysis](conservative-analysis.md)
and the [current release policy](compatibility.md). The old corpus suite remains
available as `bun run quality:research`; its old acceptance thresholds and
failed outcomes are not rewritten to claim a successful conservative release.
References below to the one-shot `release:semantic` command describe the
retired orchestrator in `scripts/run-historical-semantic-release.ts`, not the
current command.

Semath evaluates semantic behavior with separate signals rather than a blended
accuracy number. The checked-in [quality manifest](../fixtures/corpus-manifest.json)
owns suite discovery, support tiers, coverage dimensions, transforms, and
thresholds.

| Signal                   | Meaning                                                                                | It does not mean                                      |
| ------------------------ | -------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Recall                   | reviewed positive cases that recognize one target law                                  | field-wide notation coverage                          |
| Precision                | target recognitions absent from reviewed refusal cases                                 | probabilistic confidence                              |
| Role accuracy            | expected roles bind the intended symbols                                               | universal validity of the formula                     |
| Evidence integrity       | conclusions retain conditions and source ranges                                        | sufficiency outside the source context                |
| Refusal preservation     | negative cases avoid recognizing the target law                                        | proof that the input is false                         |
| Adversarial refusal      | unknown and cross-pack collision cases recognize no law                                | refusal of a different, valid law in the same formula |
| Variation coverage       | labeled notation, prose, role, constraint, project, macro, and mutation families       | equal real-world frequency                            |
| Diversity cells          | distinct semantic skeleton, syntax, prose, project topology, and mutation profiles     | real-world prevalence                                 |
| Metamorphic invariance   | irrelevant prose, comments, and document ordering preserve outcomes                    | arbitrary source rewrites are safe                    |
| Pack-derived properties  | every law receives positive, refusal, scope, mutation, macro/project, and cursor cells | production recognition agrees with itself             |
| Differential equivalence | clean, incremental, native, WASM, Worker, and LSP projections agree exactly            | a second semantic engine exists                       |
| Prose association        | declared symbols receive their exact descriptions                                      | the description is a supported domain concept         |
| Assumption extraction    | explicit assumptions retain subjects and evidence                                      | hypothetical or cited properties are assumptions      |
| Prose scope              | declarations and assumptions obey section and include order                            | later or disconnected evidence is visible             |

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
reviewed source positions across the 61-law historical baseline. Exactly eight holdout
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

Each semantic release adds a new 48-scenario blind tranche rather than reusing
the historical holdout as a tuning target. It has exactly eight scenarios in each
document-reasoning family and covers every decision class. Isolated Codex
subagents author its source and expectations without engine execution;
independent subagents critique them; then the main agent reads every source and
expectation, applies corrections, freezes the task card and review digests, and
seals both the authored fixture and its release envelope. Author, critic, and
main-review identities must be distinct, and the validator rejects source
reuse or suspicious math/prose similarity with any checked-in fixture.

Fresh execution is a final qualification step, not a debugging cadence. v0.41
uses the sealed authoring-safety envelope from release schema 2; v0.42 and later
use release schema 3 with authored-fixture schema 2. The latter gives every
probe separate cursor-entity and selected-formula decisions, with the formula
anchored to one exact syntax math root. Neither contract may contain a
reviewer-guessed complete `StableMathAuthoringContext`. Exact lifecycle and
authority boundaries remain release-blocking, while representational
projection detail is exercised by the versioned public MathAuthoring oracle
before reservation. Receipt policy 3 retains the structured authoring-safety
summary instead of flattening it into case IDs. A new tranche is not
commissioned while that oracle or a known public P0 safety regression is
failing.

The public recognition challenge applies the same split in its strict v4
overlay without rewriting the frozen v2/v3 corpus. Every case declares whether
its decision belongs to the cursor entity or selected formula. Formula
relations are reviewed separately as candidates or authoritative results;
authority requires a complete, grounded `typed` or `derived` role binding,
verified conditions, and exact selected-root evidence. Relation visibility by
itself is never authority. The overlay pins the predecessor fixture digests so
the composition cannot silently change its historical base.

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

The [practical STEM breadth benchmark](stem-breadth-benchmark.md) projects that
same reviewed public-development evidence into five program fields and ten
capabilities. Its first-loss scoring keeps vocabulary, typing, recognition,
equivalence, conditions, attachment, lifecycle, decision, navigation, and
refusal visible separately. Commissioned empty cells remain gaps until their
linked field issue adds independently reviewed evidence; a strong aggregate or
an unrelated probe cannot fill them.

The 2026-08-13 v0.28 pre-blind evaluation passes 50 of 115 editable development
probes (risk 130, no false establishment, false conflict, or navigation risk).
The immutable semantic-continuity report remains the historical target surface.
The release gate applies thirty exact, independently reviewed decision
transitions without changing its shape, Problems, relation, or identity
expectations: nineteen source-backed `partial` to `established` transitions,
ten unsupported-ambiguity `ambiguous` to `partial` transitions, and one
source-grounded derivative `ambiguous` to `established` transition. The current
release score is 44 of 48 with risk 8 and no unsafe risk; the four remaining
vector-shape misses stay visible as coverage work.

The current 2026-08-22 development frontier passes 108 of 166 probes with risk
116 and no false establishment, false conflict, identity, or navigation risk.
Its first-loss atlas is 24 attachment, 18 typed-fact, and 16 pack-unification
misses. The additional reviewed probes cover typed linear-algebra,
differential-equation, probability/statistics, and numerical-analysis relation
families plus declaration retraction and exact navigation behavior. This
supersedes the older editable-development measurement above for current release
work without changing the historical v0.28 and v0.35 records.

The now-exposed historical authored holdout passes 7 of 97 with raw risk 720.
These deliberately difficult documents are a first-loss map, not a release
pass-rate target. Its remaining raw false-establishment count is an exact,
reviewed frozen-contract disagreement: the fixture expects prose in a
disconnected project component to retract a relation established in another
component, while Semath preserves that dependency boundary. The release gate
permits only this named, proof-grounded decision, proof, and relation case
and rejects substitution by any new false establishment. It also adjudicates
one frozen cursor contract that asks an `edge: after` formula-boundary query to
select an arbitrary internal symbol; exact boundary ownership correctly returns
no symbol. The adjusted risk is 720 and adjusted identity/navigation count is
54, both within the prior gate. One additional named adjudication recognizes a
source-grounded navigation recovery for the solver tolerance `\varepsilon`:
the engine now returns its exact prose definition, all three source references,
and the selected formula occurrence for prepare-rename, while the frozen
contract expected those surfaces to be unavailable. This recovery is accepted
only when every reviewed source location matches and no rename edit is emitted;
it does not change the frozen fixture or the release thresholds. Current
historical first losses are 26
attachment, 40 identity or scope, eight typed-fact, 13 pack-unification, and
three decision cases. See
[Pack maturity](pack-maturity.md) for interpretation by capability.

The sealed v0.28 fresh blind was executed once on 2026-08-13 after every
pre-blind gate passed. It scored 7 of 48 with risk 490: four false
establishments, two false conflicts, 35 identity or navigation expectation
misses, and 34 coverage misses. Clean and incremental results agreed across all
12 reviewed lifecycle stages, and no unsafe navigation or edit was observed.
The terminal receipt is `safety-failed`, so this candidate is not a release and
must not be pinned by CorTeX. The fixture is now historical evidence and must
not be used to tune or silently relabel the engine. Future one-shot receipts
retain exact safety case IDs as well as counts and artifact digests so a failed
run remains diagnosable without executing its fixture again.

The independently sealed v0.29 fresh blind was also executed once on
2026-08-13, after the complete pre-blind gate passed on the separate release
host. It scored 4 of 48 with risk 426: nine false establishments, no conflicting
decisions but six cases exceeding their calm diagnostic limit, 20 unsafe
navigation or edit observations, and 29 coverage misses.
Clean and incremental results agreed across all eight reviewed lifecycle
stages. Its terminal receipt is `safety-failed`, so PR #304 is not a release and
must not be merged or pinned by CorTeX. The fixture seal is
`c4431c1203d56af5e0db55449ae0b72392d79c560b193a8ea6daf9ef68adffb8`;
future work must treat this tranche as historical evidence rather than a
tuning set.

The independently sealed v0.30 fresh blind was executed once on 2026-08-13
after every pre-blind gate passed. Development was 51 of 115 with risk 128 and
no false establishment, false conflict, or identity risk; open semantic safety
was 27 of 27, recognition frontier 32 of 32, and the adjudicated continuity
release score was 37 of 48 with risk 22 and no unsafe risk. The primary fresh
evaluation completed with 0 of
48 probes passing and raw risk 390: four false establishments, no false
conflicts, 26 identity or navigation misses, and 41 coverage misses. The
lifecycle runner then stopped with a document-version mismatch, so the atomic
terminal receipt is `execution-error` rather than a semantic release receipt.
The fixture seal is
`cdddd29c75891320ad6e643cb60e02b9e8cec08af26fab1facca4311b782f8f5` and the
evaluated Semath commit is `695b6d0cc3386b3ef117837baf1bd3596b49a800`.
This candidate is not a release and must not be merged or pinned by CorTeX.
The exposed fixture is historical evidence and must not be rerun or used to
tune the engine. The lifecycle planner now always forwards directly edited
documents even when wasmtex can reuse their syntax, and future execution-error
receipts retain any completed primary evaluation before recording the tool
failure.

The independently sealed v0.33 fresh blind was executed once on 2026-08-13
after its complete pre-blind gate passed. It scored 3 of 48 with risk 436:
twelve false establishments, no false conflicts, five cases with unsafe
navigation or edit results, and 41 coverage misses. All eight lifecycle stages
agreed. Its terminal receipt is `safety-failed`, so PR #320 was closed without
merge and CorTeX must not pin it. The fixture seal is
`8e457adf5e653c5edaf91d305e3d64ed673f109f8b8b3a37e719874c45add0c7`.
This fixture is historical evidence and must not be rerun or used for tuning.

The v0.34 public proof-authority adjudication preserves the 50-of-115,
risk-130 development release baseline and zero public safety risk. Eleven
reviewed probes now remain `partial` because a formula assertion or domain
context supplies useful recognition without independent typed role roots. The
open semantic safety suite has 39 metamorphic observations across nine
contracts; assertion without complete role proof is tested separately from
exact establishment.

The immutable historical release gate records one explicit v0.34 policy
adjudication: `CA-HO-06-probe` retains its source-grounded derivative relation
but is conservatively `partial` because its complete independent role proof is
not available. The gate subtracts only that exact, validated decision miss; it
does not change the fixture or widen any aggregate threshold, and it rejects
the adjudication if the source-grounded relation disappears.

The independently sealed v0.34 fresh blind was executed once on 2026-08-13.
It scored 2 of 48 with risk 308: no false establishments, no false conflicts,
44 coverage misses, and four cases containing 18 source locations outside the
reviewed navigation/edit allowlists. All 18 lifecycle stages agreed and no
diagnostic exceeded its reviewed limit. The terminal receipt is
`safety-failed`; its fixture seal is
`405a28155369fb9544712b83752a500cae8403193489eb7fb504ff110fe14c36`.
This fixture is historical evidence and must not be rerun or used for tuning.

Starting with v0.35, static commissioning requires definition and references
to share one entity authorization and prepare/rename to share one edit
authorization. An available atomic surface must enumerate every exact source
spelling; every definition must be a reference, and a rename must edit exactly
the complete reference set. This strengthens the blind oracle before any
engine execution instead of weakening the release safety gate afterward.

The independently sealed v0.35 fresh blind was executed once on 2026-08-13
after the complete release gate passed on a separate x86_64 Linux host. It
scored 0 of 48 with raw risk 570: no false establishments, no false conflicts,
45 coverage misses, and 48 identity or navigation expectation misses. The
important release boundary passed: there were zero unsafe navigation or edit
locations, zero excessive diagnostic cases, and all eight clean/incremental
lifecycle stages agreed. The terminal receipt is `completed`; its fixture seal
is `a11c0d4dcb89a3574af83b5182a566f0ca09768a90d9c50702f46ef53040f9ae`,
its evaluated Semath commit is
`67f901226a91c4f3619ebb7de58c2eda62037a44`, and the receipt digest is
`76bfbe71ca679847cf38062b4692fe65bbf1e362e04d18bf0f500408fe9d89a3`.
This is a safety release, not a claim of broad fresh-document coverage. The
exposed fixture is now historical evidence and must not be rerun or used for
case-specific tuning; coverage must continue through public development and
new independently sealed evidence.

The independently sealed v0.36 fresh blind was executed once on 2026-08-18
UTC after candidate `ed48fc4c0dda70b001754f782d3d06e547fcb2ad` passed every
pre-blind gate on x86_64 Linux. It scored 0 of 48 with risk 986: 47 false
establishments, three false conflicts, 48 coverage misses, and 29 identity or
navigation misses. The expanded authoring-context safety oracle found 192
unexpected facts across all 48 cases, 10 unsafe lifecycle transitions across
four cases, 79 navigation or edit locations outside the reviewed allowlists
across 23 cases, and three diagnostic-limit violations. All eight
clean/incremental lifecycle stages agreed, so this is a semantic safety failure
rather than a lifecycle-parity or release-infrastructure error. The terminal
receipt is `safety-failed`; its fixture seal is
`a69a5eb8a1a0147aef6494e0f40f757bcaf8a5bb46d32f7210aef91c4926419a`
and its receipt digest is
`d81e92197d914f76f5cab84c0467c39d494c26169907c469078c78c20efcf684`.
PR #365 was closed without merge. No 0.18.0 package, tag, or CorTeX pin was
published. The exposed v0.36 fixture is historical evidence and must not be
rerun, relabeled, or used for case-specific tuning.

## Protocol 17 evidence-graded development facets

On 2026-08-22, protocol 17 projected evidence-graded hypotheses over all 166
independently authored development probes. The facet report found hypotheses
in 160 cases and multiple bounded hypotheses in 127. Supporting evidence was
present in 155 cases, contradictory evidence in one reviewed Faraday-law
conflict, missing discriminators in 120, natural-language provenance in 77,
scoped-domain provenance in 147, and reviewed conventions in 11. All 166 cases
reported bounded-open-world exhaustiveness, exact file/path/range/revision/scope
anchors, and deterministic evidence-bearing ordering. The dedicated safety
checks found zero advisory authority, anchor, ordering, contradiction, or
discriminator-link failures.

These are separate facet counts, not a confidence score or completeness claim.
The reviewed authored decision baseline on the same run is 108 of 166 with
risk 116, zero false establishments, zero false conflicts, and zero unsafe
navigation or identity results. The new projection reports uncertainty and
provenance without silently improving those decisions.

The separate public MathAuthoring gate contains 20 source-authored cases in 10
meaning-matched TeX/Markdown pairs. It checks native/WASM and clean/incremental
parity, exact source identity, lifecycle fences, reviewed authority and
contradiction, cross-document evidence, generated-source limits, approximation,
and removal transitions. Eight hypotheses are release-required; 18 broader
interpretations remain explicit known misses. Candidate-cap metadata is a
protocol-owned 16/+1 unit contract, while production of more than 16 genuine
document candidates remains a known miss rather than a synthetic public E2E
claim. The canonical source, constraints, and independent attestation have raw
SHA-256 digests `f6fcdb76a456e20129a61f819d18902e422e3735a6050fc000c16ea20a300c0c`,
`a96023f0f9784816b666a348557e8539fc5f7d0a969a36fe00bbea9c1bfd68f8`,
and `84e36d6f9a5a41aa6c09b8103f1a20f125d7de3d6a8a42395a8ed42306988d52`.
The review digest is
`7693f886b6eda4c71ca29dd899aa006e03ad4a79cbede998747d8af4ac7efbe9`;
the passing content-addressed diagnostic digest is
`ce472e0a45e01dd02e289e20c3460cffe3a1b521389008a9c80aa71e12948692`.
The full diagnostic is untracked and is never an oracle input. No v0.36
fresh-blind input was read, rerun, or used for tuning.

Evaluated laws require
100% role, evidence, and refusal preservation, at least 99% precision, and at
least 95% recall. This baseline is release evidence, not a completeness claim or
live production telemetry. It contains no imported real-world corpus, so it must
not be used to claim field completeness or real-world frequency.

Full semantic corpus execution and the public MathAuthoring oracle are
deliberate manual/release gates, not default pull-request checks. Default CI
parses schemas, verifies generated assets, tests the pure scorers, enforces pack
conformance, and rebuilds the x86_64 WASM artifact. The manually dispatched
`semantic-quality` workflow then runs the public MathAuthoring release gate and
the full corpus against that rebuilt artifact. This keeps ordinary feedback
bounded without presenting expensive synthetic evaluation as continuous
production evidence.

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
bun run math-authoring:development
bun run authored:development:release
bun run authored:historical:release
bun run corpus
bun run corpus:generate:check
bun run foundation
bun run foundation:generate:check
bun run scorecard
```

The fresh blind commands are intentionally absent from the generic examples:
they require an independently commissioned, sealed release fixture and a new
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
refinements, the final manual run passes all thresholds for all 13,936 cases and
all 1,230 metamorphic observations: every one of the 136 evaluated laws has 100%
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
