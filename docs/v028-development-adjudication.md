# v0.28 development-contract adjudication

This is a dated review record for the mutable 115-probe development fixture.
It does not change or reinterpret the immutable historical or fresh holdout.

The review was performed on 2026-08-12 against commit `56e7f8f`, with the
subsequently integrated prose changes present during the final measurement.
Every accepted correction is grounded in an exact source occurrence. Formula
boundaries were not used to guess an intended symbol.

## Corrected contracts

| Probe | Old focus | Reviewed focus | Contract |
| --- | --- | --- | --- |
| `OPTML-DEV-002` | after the objective formula; resolved `R` at `340..341` accidentally | `R` at relative offset 36, absolute `340..341` | Definition `106..107`, two references, and deterministic `R` to `S` rename are available. |
| `DMD-10-established-inclusion-exclusion` | after the formula; resolved `\cup` at `301..305` | final `B` at relative offset 25, absolute `324..325` | Definition `93..94`, references, and deterministic `B` to `C` rename are available. |
| `probability-development-retry-conditional-event-intersection-given` | after the formula; resolved `P` at `145..146` | intersection operand `B` at relative offset 8, absolute `153..154` | Definition `64..65`, references, and deterministic `B` to `C` rename are available. |
| `DMD-03` | after the branch formula; resolved the defined operand `B` | `\cap` at relative offset 3, absolute `270..274` | The unresolved branch remains ambiguous; entity navigation and rename are unavailable for the operator. |
| `DMD-03-union-alternative` | after the branch formula; resolved the defined operand `B` | `\cup` at relative offset 3, absolute `302..306` | The unresolved branch remains ambiguous; entity navigation and rename are unavailable for the operator. |

The replacement names are valid within their reviewed scopes: `S` does not
collide in the regularized-objective document, and `C` does not collide in the
set or event scenarios. The fixture requires only source-grounded edits of the
selected entity; it does not prescribe display-string substitution.

## Remaining identity-stage losses

These five cases are not fixture-contract errors. Their cursors identify valid
source occurrences, while the expected decision follows the document's prose
or mathematical relation. Changing their expectations or moving their cursors
would hide an engine loss.

| Probe | Observed occurrence | Reviewed classification |
| --- | --- | --- |
| `CA-DEV-05` | `x` at `limitations.md:340..341` | Engine decision loss: the document explicitly declines to assert the candidate formula, so `unsupported` remains correct. |
| `CA-DEV-09` | `f` at `discussion.tex:145..146` | Engine discourse/conflict loss: the hypothetical gradient statement is later refuted, so `conflicting` remains correct and no entity lifecycle is promised. |
| `CFCD-04` | composite `d^2` at `observations/night_17.md:369..372` | Engine recognition/type loss: the inverse-square astronomy relation is source-grounded and should be `established`. |
| `DEV-CFS-03` | composite `(x,t)^4` at `preregistration.tex:423..430` | Engine recognition/type loss: the pressure-flow relation is source-grounded and should be `established`. |
| `EM-DEV-006` | `\nabla` at `lab/2026-05-18-coil.md:420..426` | Engine discourse/conflict loss: prose rejects the plus-sign Faraday equation, so `conflicting` remains correct. |

No remaining development failure reports a definition, references,
prepare-rename, rename, cursor-occurrence, or scope mismatch. The five entries
above are first-loss taxonomy outcomes caused by their still-incorrect semantic
decisions, not evidence that their contracts should be weakened.

## Second exhaustive cursor review

The 89 remaining misses were reviewed again for formula-boundary, punctuation,
metadata, operator, and entity-focus contract errors. Formula trailing edges
are intentional ownership probes: moving them mechanically to `=`, `:=`, or a
relation head caused one false establishment and nine identity or navigation
regressions. Formula meaning and entity navigation therefore must remain
separate evaluator surfaces; the fixture must not hide engine cursor losses by
moving those cursors.

One semantic expectation was mathematically invalid. The flywheel source uses
`K=\\tfrac12 mv^2` without asserting the pack's required nonrelativistic-motion
condition. `cm-development-flywheel-review` now expects a partial, ungrounded
decision. Its relation contract covers only kinetic energy: the separate
momentum relation starts after this probe's cursor evidence boundary and cannot
be required by the same query. No cursor or navigation expectation was
weakened.

## Verification

The fixture validator passes with 96 development scenarios, 115 development
probes, and unchanged frozen-holdout integrity. The development run reports
24/115, risk 182, false establishment 0, false conflict 0, and navigation or
identity risk 0. The remaining 91 misses are engine coverage losses.
