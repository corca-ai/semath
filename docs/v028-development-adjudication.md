# v0.28 development-contract adjudication

This is a dated review record for the mutable 115-probe development fixture.
It does not change or reinterpret the immutable historical or fresh holdout.

The cursor-contract review began on 2026-08-12 and the final convergence review
was completed on 2026-08-13. Every accepted correction is grounded in an exact
source occurrence. Formula boundaries were not used to guess an intended
symbol.

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

## Exhaustive cursor review

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

## Final convergence review

The final development-only review was performed on 2026-08-13. It did not load
or execute either the historical or fresh holdout. The engine passes 50 of 115
probes with risk 130, false establishment 0, false conflict 0, navigation or
edit risk 0, and identity-contract risk 0. This is a coverage measurement, not
a release threshold or a claim of field completeness.

Every remaining miss was retained after source, expectation, and first-loss
review. The groups below are accepted bounded v0.28 limits; they are not
relabelled successes and their fixture contracts remain unchanged.

| First loss | Count | Reviewed bounded limit |
| --- | ---: | --- |
| Attachment | 25 | The source requires relation commentary, later refutation, alternatives, derivation references, or cross-sentence condition attachment beyond the closed typed constructions admitted in v0.28. Nearest-text or broad asserted-formula fallbacks produced unsafe establishment and were rejected. |
| Identity | 3 | The cursor owns a real composite/operator occurrence, while the expected decision depends on formula-level scientific meaning. Guessing a neighboring symbol would violate the exact occurrence contract. |
| Typed fact or condition | 23 | The relation skeleton or decision is often present, but a domain, shape, context, sign, or role fact is not proven at the exact source occurrence. Vocabulary alone is not proof. |
| Pack unification | 13 | The formula needs a still-missing generic equivalent form, composite role binding, guarded dispatch, or source projection. Per-law Rust branches were rejected. |
| Decision | 1 | The document contains a rejected calculation and a later accepted calculation; the remaining query needs exact source-linked opposition rather than document-wide conflict inference. |

The retained probe IDs are recorded here so the dated score is reproducible:

- Attachment: `CA-DEV-09`, `CFCD-01`, `CFCD-03`, `CFCD-06`,
  `CFQ-DEV-001`, `DEV-CFS-05`, `DMD-03`, `DMD-04`, `DMD-05`,
  `DMD-09`, `DMD-10`, `EM-DEV-007`, `FM-DEV-006`, `OPTML-DEV-002`,
  `OPTML-DEV-003`, `OPTML-DEV-005`, `OPTML-DEV-007`, `OPTML-DEV-008`,
  `thermo-dev-005`, `thermo-dev-008`, `thermo-dev-009`,
  `DMD-03-union-alternative`, `DMD-05-accepted-directed-identity`,
  `DMD-10-established-inclusion-exclusion`, and
  `probability-development-retry-conditional-event-intersection-given`.
- Identity: `CFCD-04`, `DEV-CFS-03`, and `EM-DEV-006`.
- Typed fact or condition: `cm-development-impulse-cart`,
  `cm-development-power-symbol-collision`, `control-systems-development-03`,
  `CFCD-02`, `CFQ-DEV-004`, `CFQ-DEV-006`, `DEV-CFS-06`, `EM-DEV-002`,
  `EM-DEV-009`, `FM-DEV-004`, `FM-DEV-007`, `FM-DEV-008`,
  `OPTML-DEV-001`, `probability-development-metric-name-collision`,
  `thermo-dev-002`, `thermo-dev-004`, `thermo-dev-006`,
  `CA-DEV-10-established-specialization`,
  `OPTML-DEV-010-established-stationarity`,
  `probability-development-metric-name-collision-accepted-variance-scaling`,
  `FM-DEV-008-stagnant-cell-fick-law`,
  `probability-development-metric-name-collision-established-expectation-linearity`,
  and `cm-development-power-symbol-collision-lowercase-momentum-after-definition`.
- Pack unification: `CA-DEV-01`, `CA-DEV-07`, `CA-DEV-08`,
  `control-systems-development-01`, `control-systems-development-02`,
  `EM-DEV-001`, `EM-DEV-004`, `FM-DEV-005`, `FM-DEV-009`,
  `thermo-dev-003`, `OPTML-DEV-009-stopping-rule`,
  `probability-development-release-overlap-accepted-union`, and
  `thermo-dev-010-established-after-role-declarations`.
- Decision: `probability-development-release-overlap`.

The implementation tranches that reduced the original risk preserve one
identity authority, one typed discourse path, one canonical IR and generic
unifier, and one decision projection. Superseded binder-only rename, raw
active-definition matching, query-time entity guessing, broad formula
establishment, and unsafe base-symbol rename paths are absent. Full core tests,
clippy, development fixture validation, and documentation lint are required
again at the release commit.
