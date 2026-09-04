# Spent holdout postmortems

This document is a historical record of the terminal v0.38 through v0.43 semantic
release evaluations. It is not a current scorecard, a release plan, or a source
of expected answers for another holdout.

All six fixtures are permanently spent. They must not be rerun, edited into a new
holdout, or used as answer templates. Public regressions derived from them use
new source text and the lowest authoritative public test layer.

## Immutable evidence

| Release | Run | Artifact | Candidate | Fixture SHA-256 | Evaluation SHA-256 |
| --- | --- | --- | --- | --- | --- |
| v0.38 | `32462617124` | `9440104778` | `1c11dcafc8801df0a34cf9f61829ef843dbc8534` | `fb1ced3d602a32ef697022f36bc67a57ca0fc4b5fe014eff2bedcf4314f3aa94` | `de733715718f3e6f8093ce0fac6101c277620ff2672536ceef857c53675b2cdb` |
| v0.39 | `32571193980` | `9475579401` | `b61758bf3783954f2a4b057aabc048c3b0f913ad` | `68a23c8d1135e80d25c31c72e8136e6a436da0ea9d0d4748c21741a4913befce` | `d6ed5e53b7fc56372b8088df117aca11a0eb1792f1399767de3516f48b0f9103` |
| v0.40 | `32730705424` | `9522100540` | `1fea214aa45224cc1767047dd78f46b02292183e` | `2e32389b8386845ee8ce491b3fc0a3fa55114abc3b1351c0f29152fba7a92e17` | `789011bea34fba2c4249231f9c70c7cebac6197b18b9c8c7447ce9c2abd1544d` |
| v0.41 | `32800370262` | `9546760981` | `15ca913b4a19c81e3ad3d6a6054bee1059a7561f` | `1bf5870f1a8555a425061a9b280897f0ae0fb703e0b47625d8621e08bbda1b59` | `1f89a11044dc33ce43cf16f8d52046204dcd05ee171d9090eff62cd464ca4b12` |
| v0.42 | `32820656318` | `9553675078` | `db15c0bacbc0d9f8b9a247fb6c6132481e28448c` | `672154b236f9fc73d4d0c5d24ee225ef2949f0ada72da5c6757a68fa545d0d34` | `d2edad8f07ac80b0f8a3f47a6718ea685047e68f29b95e3fc35e380d052276a1` |
| v0.43 | `32925794681` | `9591749329` | `69eda99dffdd776101ed7554939823de26226687` | `9325d50f44fcf4d2c5460acc20a7bcbdd2e5a4a26474c188a9868600d4b81ebd` | `afabea9f369ed842ed86a28201ffb0f9157631a0a09e607d49c450c010d20f34` |

Each run passed the pre-reservation gates, permanently reserved its release
identity, executed the engine once, terminalized, and retained its result. The
low scores were semantic evaluation outcomes, not retryable workflow failures.

## What the raw scores hid

| Release | Passed | Risk | Mechanical oracle or commissioning | Evaluator contract or diagnostic | Engine safety | Engine coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| v0.38 | 1/48 | 488 | 16 | 2 | 4 | 25 |
| v0.39 | 2/48 | 220 | 9 | 1 | 6 | 30 |

The v0.38 classification covers its 47 failed top-level probes. The v0.39
classification covers its 46 failed top-level probes; its two top-level passes
still had non-exact strict authoring contexts. These are adjudicated primary
causes, not counts emitted directly by the scorer.

The raw results therefore do not mean that the engine had 47 or 46 independent
defects. They combined four layers:

1. A malformed or stale expected surface.
2. A scorer or diagnostic that grouped unlike signals.
3. Unsafe engine authority, such as an excluded relation leaking or an
   unsupported expression becoming established.
4. Safe incompleteness, such as a missing attachment, role binding, or pack-law
   relation.

## Lessons retained from v0.38

The dominant commissioning defect was an exact authoring oracle that did not
match the public projection contract. All 48 authoring contexts differed and
produced 2,619 findings. Repeated causes included a trimmed inner equation where
the protocol owns the display content root, handwritten document version 2 for
a standalone version-1 observation, noncanonical ordering and generated group
ordinals, and empty scope paths where the syntax projection owned a section.

The scorer also mixed cursor-symbol decisions with formula authoring
dispositions. An established symbol inside a partial formula was counted like
an established formula. Its false-establishment count combined five decision
signals with three excluded-relation leaks, while a historical helper attempted
to reconstruct the same count from decisions alone.

Four cases did demonstrate safety defects: an incompatible Newton relation
leaked, a guarded probability relation established without its condition, a
conflicting thermodynamic relation leaked, and a retracted independence
relation survived a lifecycle change. Twenty-five failures were primarily
coverage gaps.

## Lessons retained from v0.39

Mechanical noise fell, especially the top-level navigation/identity risk, but
the strict authoring oracle still had 0/48 exact cases and 433 findings. Empty
expected arrays were being read as authoritative absence even where the public
producer emitted a larger stable context. Several requested `ambiguous`,
`conflicting`, or `unsupported` decisions lacked two source-backed hypotheses,
contradicting evidence, or explicit refusal evidence in the terminal record.

Six scored false establishments were confirmed safety candidates: excluded
Newton, Ohm, and kinetic-energy relations leaked in reviewed negative cases; an
undeclared structural power expression established without asserted source
meaning; and a temperature symbol established with no claim evidence while its
declaration discriminator remained outstanding. One other establishment flag
was an evaluator error because the selected entity had a hard source claim even
though the named law was missing. Navigation risk meanwhile mixed two compound
identity misses, two noncanonical operator-wrapper expectations, and one
directionless navigation mismatch rather than proving five unsafe leaks.

The batch also reported zero contradiction cases. A batch commissioned to test
conflict must contain at least one public-contract-valid hypothesis with exact
contradicting evidence; merely writing `expected: conflicting` is insufficient.

## Lessons retained from v0.40

The v0.40 run passed reservation and artifact checks, then completed as
`safety-failed`: 9 of 48 cases passed with risk 302. The retained result reported
10 false establishments, one false conflict, 30 coverage misses, and 11
navigation or identity failures. Infrastructure was not the cause: syntax was
available for every case, the engine was not limited, lifecycle evidence was
retained, and committed and rebuilt WASM digests agreed.

Mechanical formula roots, UTF-16 ranges, and section scopes were correct, but
the strict authoring projection was still 0 of 48 exact with 3,338 findings.
Reviewers had been required to predict the producer's complete stable
hypothesis, requirement, evidence, and anchor graph without executing it. That
is not a valid blind commissioning task. Most differences were representational
and hid 19 high-severity authority, conflict, or lifecycle findings.

Starting with v0.41, a fresh fixture must use release-envelope schema 2. It
forbids a guessed complete `StableMathAuthoringContext` and instead seals a
bounded authoring-safety contract: exact lifecycle, forbidden dispositions,
and exact allowlists for mathematical authority and contradictions. Intrinsic
anchor, ordering, authority, and lifecycle invariants remain release-blocking.
Missing semantic coverage remains visible through the ordinary score rather
than being disguised as thousands of internal-object diffs.

## Lessons retained from v0.41

The first schema-2 run passed every pre-reservation gate and completed normally
as `safety-failed`. It scored 5 of 48 with raw risk 498: 14 reported false
establishments, two reported false conflicts, 28 coverage misses, and 25
navigation or identity mismatches. The retained lifecycle comparison covered
all eight selected transitions, and the committed and rebuilt WASM digests
agreed, so infrastructure was not the cause.

The bounded safety envelope exposed real defects that the ordinary score did
not: two withdrawn formulas remained current and editable, no reviewed conflict
produced contradictory evidence, and one alternative-shape case emitted
authority and a diagnostic that belonged to neither selected alternative.
Those are release blockers and require new public synthetic regressions.

The run also revealed evaluator and commissioning defects. Seven condition-
missing or conventional relation candidates were counted as established merely
because `RelationInfo` was present. Diagnostic-limit failures were counted as
false conflicts even for a reviewed conflict. Twenty probes put the cursor on
`=` and asserted that no occurrence owned it, while the published cursor
contract assigns a nonempty left trailing edge to the left occurrence. Finally,
one decision field still mixed a cursor entity's meaning with the enclosing
formula's disposition. These signals must be separated rather than fixed by
weakening the safety envelope.

Starting with v0.42, commissioning must distinguish cursor-entity decisions
from selected-formula decisions, retain relation authority rather than infer it
from relation presence, and record structured authoring-safety failures in the
terminal receipt. A new fixture may be commissioned only after the corresponding
public evaluator and engine regressions pass. v0.41 remains immutable spent
evidence and must not be rerun.

## Lessons retained from v0.42

The schema-3 run passed its static, reservation, lifecycle, artifact, and
attestation boundaries, then completed normally as `safety-failed`. It scored
1 of 48 with raw risk 522: two reported false establishments, 44 coverage
misses, and 41 navigation or identity findings. The committed, rebuilt,
retained, and packaged WASM digests agreed, all 48 selected formula anchors
matched, and no observation was engine-limited. Infrastructure was not the
cause.

Commissioning still confused source syntax with semantic occurrence identity.
Thirty-eight probes selected a nucleus inside an indexed, decorated, styled, or
otherwise composite notation while expecting a flat base symbol or an enclosing
formula. The public cursor contract preserves the owned composite occurrence.
Those malformed expectations dominated the navigation score and also made six
source-grounded navigation results look unsafe. Future static validation must
derive the selected occurrence from neutral syntax facts and reject a fixture
that uses its own expected symbol as the oracle.

The new entity/formula fields exposed a separate engine boundary rather than
fixing it. Formula locations were exact in all 48 probes, but formula status was
exact in only 12. The engine still derived the formula authoring disposition
from the cursor entity's `MeaningDecision`, so independently reviewed
ambiguous, conflicting, and unsupported formula states collapsed mostly to
partial or conventional. Formula-root adjudication and cursor-entity
adjudication must have separate inputs, conflicts, and evidence projections;
neither result may authorize the other.

Two retained cases remain genuine public safety work. One nonselected
convention still emitted a supported typed-law relation. One explicitly denied
formula root still generated positive canonical-symbol identity evidence.
Separately, none of the reviewed conflict cases projected exact contradicting
evidence. These findings require newly written public tests for nonselected
conventions, exact-root refusal propagation, and formula-owned contradiction.
They must not be addressed by lowering the contradiction or authority gates.

Before v0.43 commissioning, public gates must therefore prove independent
entity/formula decisions, syntax-backed composite cursor ownership, exact
negative-frame propagation to nested identities, non-authoritative conventional
candidates, and anchored formula contradictions. v0.42 remains immutable spent
evidence and must not be rerun.

## Lessons retained from v0.43

The schema-3 run completed as `safety-failed`: 7 of 48 probe scores passed,
with risk 204, and 44 cases had at least one authoring-safety finding. One
ambiguous conditional-probability root was genuinely over-established because
condition-name tokens and formula shape were treated as verified preconditions.
Public engine regressions now cover that authority boundary.

The remaining output separated into useful development work rather than one
undifferentiated failure. Explicit incomplete candidates were often lost as
unsupported or conventional; unresolved alternatives collapsed instead of
remaining ambiguous; reviewed negative roots lacked anchored contradiction;
and independent entity declarations did not authorize navigation. The scorer
also called conservative refusals unsafe and often attributed the first loss to
formula disposition after an earlier source-meaning or condition failure.

Two lifecycle expectations were not derivable from the structured fixture at
all: generation and retraction existed only in prose and the out-of-band oracle.
Future lifecycle evaluation must provide authoritative structured input or
treat prose only as evidence. v0.43 is now in the machine-readable spent
registry, while its generic findings remain available for development.

## Public regression atlas

The following regression families are safe to promote. Tests must use newly
written public source, not holdout sentences, anchors, IDs, or observed output.

| Priority | Public regression family | Lowest authoritative assertion |
| --- | --- | --- |
| P0 | Formula-root commissioning | TeX and Markdown multiline displays derive exact root range, notation, scope, and version from syntax facts before sealing. |
| P0 | Decision-domain separation | An established cursor entity inside a partial formula is scored as two different contracts. |
| P0 | Formula-root adjudication | Cursor-entity proof cannot establish or conflict the selected formula; formula-owned evidence cannot authorize entity navigation or edits. |
| P0 | Syntax-backed cursor oracle | Static commissioning derives the exact composite occurrence from neutral syntax instead of trusting a fixture-authored base symbol. |
| P0 | Risk subtype consistency | Decision, proof, source-grounding, and excluded-relation signals share one typed case record; no helper reconstructs a different ID set. |
| P0 | Guarded authority | A missing declaration or law condition cannot yield established or proof-grounded output. |
| P0 | Conflict authority | Incompatible source evidence yields an anchored conflict and exports no established relation. |
| P0 | Lifecycle retraction | Removing or weakening authorizing evidence retracts the relation, proof, navigation, and edit authority together. |
| P0 | Navigation scope | A shadowed or out-of-scope entity exposes no definition, reference, or rename location outside its reviewed scope. |
| P0 | Structural refusal | An undeclared power or opaque operator remains partial or unsupported and never receives source proof. |
| P0 | Root-scoped refusal | A rejected formula root gives every nested implicit identity non-positive evidence without contaminating adjacent roots. |
| P1 | Attachment | Explicit discourse attaches to the intended local or cross-document formula without leaking to a neighboring formula. |
| P1 | Composite identity | Command-token cursors project the owned compound entity, such as a prefixed quantity, rather than only its command token. |
| P1 | Pack unification | Canonical public laws bind equivalent formula shapes, indexed roles, and required conditions at exact ranges. |
| P1 | Ambiguity and dispatch | Ambiguous requires at least two independently supported candidates and deterministic discriminator groups. |
| P1 | Macro and edit identity | Transparent macros and snapshot edits preserve canonical entity identity and re-anchor all source evidence. |

The existing public formula-root, canonical-order, dot-derivative ownership,
imperative-refusal, MathAuthoring, and continuity tests are part of this atlas.
New engine tests should be added only when a fresh public reproduction isolates
one missing boundary.

## Commissioning policy after these failures

The next holdout uses the following lightweight boundary:

1. Authors and reviewers receive public regression IDs and commissioning rules,
   never terminal scenario text, anchors, per-case findings, or expected output.
2. The static gate derives formula roots, versions, scope paths, ordering, and
   generated ordinals from the public producer contract before reservation.
3. v0.41 and later forbid a hand-authored complete authoring context. Every
   probe instead has a sealed safety envelope covering lifecycle, forbidden
   dispositions, and allowed or required authority and contradictions. The
   public MathAuthoring oracle must pass before reservation.
4. Conflict coverage requires at least one independently grounded public-shaped
   contradiction case before the batch can be sealed.
5. The spent registry rejects reused release, batch, scenario and probe IDs,
   scenario raw digests, exact document digests, and suspicious prose/math
   similarity. Only SHA-256 lineage profiles are checked; spent source and
   terminal answers are not opened during commissioning.
6. Oracle, evaluator, engine-safety, engine-coverage, and infrastructure outcomes
   are recorded separately. Only a valid sealed oracle plus a green evaluator
   can support an engine conclusion.
7. There is no retry-until-pass loop. Reservation still makes every executed
   fixture permanently spent regardless of success, failure, or infrastructure
   interruption.
8. A new holdout is not commissioned while a known public P0 safety regression
   or release-oracle defect remains. Fresh execution is final qualification,
   not a debugging loop.

The machine-readable [`spent-holdout-registry-v1.json`](../fixtures/challenge/spent-holdout-registry-v1.json)
stores those lineage profiles.
`bun run spent-holdout:check` validates its closed shape, and
`bun run fresh-blind:validate` compares a proposed fixture with every registered
spent profile before reservation. The registry is an isolation aid, not a
semantic oracle and not evidence that a short hashed phrase is confidential.
Its algorithm names and public golden vector make extractor changes explicit.
As a lightweight heuristic, it intentionally cannot compare prose shorter than
five visible words or common math alone; exact document and raw-digest checks
still apply to those cases.
