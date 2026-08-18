# Pack maturity report

This is measured repository state on 2026-08-15, not live production telemetry
and not a future plan. The [quality manifest](../fixtures/corpus-manifest.json)
holds approved support policy; GitHub issues hold planned work.

## Evidence

- 13,880 scored law cases cover 136 laws in thirteen formula packs, including
  48 positive and 32 refusal diversity cases for every promoted law. Of all
  13,936 scored corpus cases, 420 remain readable fixture seeds and 13,516 are
  deterministic cases reproduced in memory from compact specs, packs, and
  seeds; generated cases are not called independently authored evidence.
- 1,230 deterministic metamorphic cases preserve results under irrelevant prose,
  comments, and document ordering.
- All 136 evaluated laws score 100% for recall, precision, role binding,
  source-linked evidence, and refusal preservation across 13,936 cases. The 1,230
  metamorphic observations also pass. Generated breadth remains regression
  evidence rather than independently authored language.
- The quantities/units foundation passes 52 of 52 non-law cases for quantity,
  unit, dimension, propagation diagnostics, notation, prose, and role evidence.
- The shared scientific-kernel and scientific-prose foundations pass 166 of 166
  positive and refusal cases across explicit operators, typed application,
  namespaced concepts, include order, and transparent or opaque prose macros.
- Every formula pack meets evaluated policy for its declared vertical. The six
  previously narrow fields now add measured constitutive/wave, heat-transfer,
  transport, nabla, graph/counting, and objective/constraint clusters. These
  are useful verticals, not claims of field completeness.
- Linear algebra now promotes 25 typed relation families rather than three.
  The added vertical covers matrix arithmetic and operators, inner products and
  norms, eigenstructure, positive-definite forms, LU/QR/Cholesky/SVD
  factorizations, pseudoinverse application, rank-nullity, basis expansion, and
  projection. Least-squares normal equations remain owned by the optimization
  pack and are reused rather than duplicated. Each of the 22 added families has
  five compact reviewed positive seeds, five semantic-mutation refusals, and a
  48-positive/32-refusal generated diversity matrix. Common inverse, adjoint,
  diagonalization, and quadratic-form presentations remain additional guarded
  representations; the promoted canonical forms are the variants for which
  exact typed source evidence passes the generic matcher.
- Differential equations now promote fourteen typed model and condition
  families: first- and second-order ODEs, linear ODEs and systems, diffusion,
  Poisson, Laplace, conservation form, initial values, Dirichlet, Neumann,
  Robin, interface continuity, and differential-operator eigenproblems. The
  pack reuses the electromagnetic wave and Helmholtz laws, the control-system
  state law, fluid-mechanics diffusivity, and linear-algebra eigenvalues rather
  than duplicating them. Its explicit-activation capability keeps bare generic
  derivatives and equalities unpromoted unless the local document declares the
  model or its roles. The 1,260 model-specific positive and refusal cases and
  126 metamorphic observations all pass; removing the declaration from an
  authored project retracts the relation without producing an unsafe edit.
- Probability and statistics now promote 22 typed relation families. The
  seventeen additions cover expected values, variances and covariances,
  density/mass normalization, CDF construction, likelihood and log likelihood,
  sample estimators and uncertainty, covariance matrices, linear regression,
  and bounded stochastic state and autocovariance models. They reuse calculus
  binders, linear-algebra shapes, and control-system state roles rather than
  duplicating those foundations. Generic `P`, `E`, `X`, `p`, and `N` spellings
  remain candidates rather than proof: local statistical roles, a named law,
  and each law's exact conditions determine promotion. The focused 1,530-case
  probe/diversity suite and its 153 metamorphic observations pass, while the
  full probability depth suite now covers 1,760 cases. An authored multi-file
  project establishes ten reviewed statistics surfaces and retracts the
  expected-value relation when its role-declaration include is removed.
- Numerical analysis now promotes 22 typed relation families across error and
  convergence, nonlinear solves, quadrature, finite differences, time stepping,
  iterative residuals, least-squares approximation, interpolation, conditioning,
  and discrete models. It reuses calculus and linear-algebra concepts instead of
  duplicating their meaning. Approximation is a distinct directional relation:
  it never enters exact-equality closure, while asymptotic order is represented
  as membership in a bound class rather than equality. Its focused 1,980-case
  probe/diversity suite and 198 metamorphic observations pass. Eleven reviewed
  document probes keep unmet numerical conditions partial and verify exact
  declaration retraction and navigation.
- The independent frozen challenge v3 preserves 48 semantic boundaries across
  binding, constraints, packs, presentation, resolution, and syntax, then
  composes them into six realistic document shapes. The pre-change baseline
  passed 33 of 48; the current engine passes 48 of 48 without changing the
  fixture. Full execution remains a manual release gate.
- The guarded-equivalence challenge freezes 24 separately authored orientation,
  scalar permutation, isolation, reciprocal, grouping, and noncommutative
  refusal cases. The pre-change `a978d2e` baseline passed 21; the current engine
  passes 24. Schema and coverage checks remain fast CI tests, while full engine
  execution is a manual release gate.
- The scoped-domain challenge freezes 30 independently authored document cases
  covering neutral fields, section scope, mixed fields, non-evidence,
  formula-before/after attachment, ambiguity, conflict, and retraction. Protocol
  10 could not represent tiered domain relevance or relevance-ordered
  alternatives; protocol 13 passes all 30 cases. Six cases independently cover
  every current structural collision component with typed formula contexts.
- The recognition-frontier v1 fixture freezes 32 stage-labeled cases across
  eight notation, discourse, lifecycle, safety, mathematics, and engineering
  families. The current engine passes 32 of 32 with zero false establishment,
  false conflict, or missed-coverage risk under the reviewed targets.
- Final stable x86_64 full-path measurements use five isolated processes per
  size and retain every raw sample. At 61 documents, median peak and live
  retained RSS growth are 125,005,824 and 90,054,656 bytes, edit p95 is 8.60
  ms, and semantic-view p95 is 1.85 ms. At 501 documents, the corresponding
  values are 155,803,648 and 152,989,696 bytes, 29.06 ms, and 3.10 ms. The
  same-host `956d89c` baseline records 7.30/27.06 ms edit p95 and
  116,846,592/149,200,896 bytes retained RSS; semantic-only edits record
  7.92/25.29 ms versus 9.30/29.30 ms. All approved limits remain unchanged. The
  stored semantic entity count falls from 1,010/8,347 to 1,007/8,291 despite
  broader source-grounded claims. Dispatch visits 1,385 and 11,501 law rules at
  61 and 501 documents, below the unchanged 24-per-document caps; the release
  WASM is 3,560,914 bytes.
- The v0.27 authored document baseline was run manually against Semath commit
  `0380421` after its 96 development scenarios were reviewed and its 48 holdout
  scenarios were separately reviewed and frozen before engine execution. The
  baseline passed 7 of 115 development probes with risk 484 and 4 of 97 holdout
  probes with risk 732. The v0.28 pre-blind release passes 50 development probes
  with risk 130 and 6 historical holdout probes with risk 720. Development has
  no false establishment, false conflict, or navigation risk. The frozen
  holdout has two exact, reviewed source-grounded contract disagreements. One
  asks prose in a disconnected file to retract an established relation in
  another component; preserving the component boundary is safer. The other
  expects a partial decision where the source directly states that an overlap
  is \(A\cap B\). The release policy names these cases and rejects any new or
  ungrounded substitution. The current v0.36 pre-blind comparison remains 6 of
  97 with raw risk 742. After the existing conservative-decision and exact
  formula-boundary adjudications, one additional exact adjudication preserves
  source-grounded navigation for the explicitly declared solver tolerance:
  one definition, all three references, and its prepare-rename range must match
  the reviewed source locations, while rename remains unavailable. Adjusted
  risk remains 720 and adjusted identity/navigation remains 54. The current
  first losses localize as follows:

  | First loss                       |   Development |       Holdout |
  | -------------------------------- | ------------: | ------------: |
  | Neutral syntax                   |             0 |             0 |
  | Prose attachment                 |            22 |            26 |
  | Identity or scope                |             0 |            40 |
  | Canonical IR                     |             0 |             0 |
  | Typed fact or condition          |            19 |             8 |
  | Local-to-observation propagation |             0 |             0 |
  | Pack unification                 |            17 |            13 |
  | Decision                         |             0 |             4 |
  | Host projection                  | Not exercised | Not exercised |

  The scorer reuses the existing recognition-frontier signals and queries
  each reviewed relation at its original equation. A low propagation count
  does not by itself prove that propagation is complete. The measured order of work is
  identity/scope, local pack and prose recognition, then canonical or typed
  gaps. Host parity remains release evidence rather than a native-baseline
  inference.

- The independently sealed v0.28 fresh blind was run once after all pre-blind
  gates passed. It scored 7 of 48 with risk 490, including four false
  establishments and two false conflicts; all 12 lifecycle comparison stages
  agreed and unsafe navigation/edit was zero. The candidate is therefore not a
  release. This exposed tranche remains historical evidence and is not a tuning
  set.
- The independently sealed v0.29 fresh blind was run once after its complete
  pre-blind gate passed. It scored 4 of 48 with risk 426, including nine false
  establishments, six cases exceeding their calm diagnostic limit, and 20
  unsafe navigation or edit observations; all eight lifecycle comparison
  stages agreed. Its terminal receipt is `safety-failed`, so PR #304 is not a
  release and CorTeX must not pin it. This exposed tranche is now historical
  evidence and is not a tuning set.
- The independently sealed v0.30 fresh blind completed its primary evaluation
  at 0 of 48 with raw risk 390, including four false establishments, no false
  conflicts, 26 identity or navigation misses, and 41 coverage misses. Its
  lifecycle comparison then stopped on an evaluation-tool document-version
  mismatch, so the terminal receipt is `execution-error`. The candidate is not
  a release, must not be pinned by CorTeX, and the exposed fixture is now
  historical evidence rather than a tuning set.
- The independently sealed v0.33 fresh blind scored 3 of 48 with risk 436,
  including twelve false establishments and five cases with unsafe navigation
  or edit results. All eight lifecycle stages agreed. Its terminal receipt is
  `safety-failed`; PR #320 was closed, CorTeX must not pin it, and the exposed
  fixture is historical evidence rather than a tuning set.
- The v0.34 public candidate separates typed, derived, asserted, and candidate
  role support and retains the actual declaration, shape, or quantity roots.
  Eleven development expectations were reviewed as `partial` where only
  formula or domain context was available; the release baseline remains 50 of
  115 with risk 130 and zero unsafe risk.
- The 2026-08-14 v0.35 development frontier connects exact formula ownership,
  source-backed formula facts, typed discourse metadescriptions, and typed
  operator assignments. Characterized operator roles now prove their matching
  domain condition without a second authority. It passes 56 of 115 probes with
  risk 118 and zero false establishment, false conflict, or identity risk.
- The practical-STEM benchmark was commissioned at
  `16e1797286fd5143144e78657230cebedd433943`, where it passed 57 of 115
  authored development probes with risk 116. The current 2026-08-15 state
  passes 108 of 166 with the same risk and zero false establishment, false
  conflict, identity, or navigation risk. Its field-by-capability matrix now
  references 104 unique reviewed probes across all 50 cells. Linear algebra,
  differential equations, and probability/statistics now measure all ten
  capabilities with no commissioned gaps. Of those 104 probes, 74 pass every
  reviewed public surface. Layer-aware cell totals are:

  | Program field            | Passed | Cases |
  | ------------------------ | -----: | ----: |
  | Shared foundations       |     20 |    31 |
  | Linear algebra           |     43 |    50 |
  | Differential equations   |     41 |    53 |
  | Probability/statistics   |     41 |    52 |
  | Numerical analysis       |     57 |    74 |

  | Capability            | Passed | Cases |
  | --------------------- | -----: | ----: |
  | Vocabulary            |     26 |    30 |
  | Typing                |     24 |    27 |
  | Relation recognition  |     30 |    41 |
  | Equivalent forms      |     11 |    19 |
  | Conditions            |     22 |    30 |
  | Document attachment   |     24 |    27 |
  | Project lifecycle     |     11 |    12 |
  | Decision quality      |     19 |    29 |
  | Navigation            |     17 |    22 |
  | Refusal               |     18 |    23 |

  These overlapping diagnostic projections cannot be summed into an accuracy
  score. They establish the ordered coverage frontier for #347–#351 while the
  separately commissioned sealed evaluation remains unwritten and unspent.

These numbers describe the synthetic benchmark, not real-world prevalence or
field completeness.

## Gaps resolved in this evaluation

| Category                    | Finding                                                                                                                | Resolution                                                                                                                                                                |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pack data                   | Linear algebra lacked matrix product/transpose; probability lacked union; three vocabulary packs had no measured laws  | Added typed pack laws and independently seeded positive/refusal families                                                                                                  |
| Generic prose               | Common plural declarations such as “events,” “sets,” and “iterates” lost roles                                         | Law-role vocabulary now derives from pack concepts and handles singular/plural wording                                                                                    |
| Generic prose               | “In this setting, let X and Y …” lost the first coordinated declaration                                                | Coordinated declarations now accept an introductory clause without a pack branch                                                                                          |
| Fixture quality             | Several seeds used invalid TeX words such as `alpha` or semantically invalid “respectively” phrasing                   | Corrected authored seeds; generation and duplicate/integrity gates prevent silent drift                                                                                   |
| Maturity policy             | One pack-wide tier conflated vocabulary, laws, typing, and provenance                                                  | Manifest schema 3 declares all seven capabilities separately and derives the summary                                                                                      |
| Foundation evidence         | Quantity/unit support was judged by a zero-law count                                                                   | Added a strict 46-case non-law corpus and pure scorer                                                                                                                     |
| Generic IR                  | Union/intersection were implicit and function calls were narrowly recovered                                            | Added explicit set/relation operators, ordered composition, and multi-argument application on one canonical path                                                          |
| Concept identity            | Runtime roles could collide as unnamespaced suffixes                                                                   | Protocol 13 and the core use pack-qualified concept identities without a legacy role field                                                                                |
| Project environment         | Included facts only affected law matching and dropped rich quantity facts                                              | One ordered external type environment now exports complete role, shape, quantity, unit, dimension, and evidence records                                                   |
| Macro semantics             | A macro command name could be mistaken for prose meaning                                                               | Only wasmtex-approved transparent surfaces contribute prose quantity meaning; opaque calls refuse                                                                         |
| Engineering composition     | Shared notation could activate an unrelated field                                                                      | Added typed role/quantity composition and a 16-case cross-field refusal suite                                                                                             |
| Pack vocabulary             | Natural concept paraphrases required runtime vocabulary edits                                                          | Schema 8 retains reviewed concept aliases consumed by the generic classifier                                                                                              |
| Constraints                 | Side conditions were free-form strings without machine-checkable subjects or evidence                                  | Schema 8 uses closed constraint kinds, validated law roles, bound symbols, source evidence, and explicit resolution status                                                |
| Generic calculus IR         | Integrals, partial derivatives, nabla applications, and indexed families lost operator structure                       | The shared structural path now keeps explicit differential variables, derivative order, integral bounds, and base/index components without command-specific pack logic    |
| Source notation             | Styled, decorated, and declared operator surfaces collapsed to a leaf or failed at a trailing application edge         | Exact wasmtex identity ranges and precomputed complete-application boundaries preserve source notation without retaining the syntax arena at query time                   |
| Prose declarations          | Elided parallel declarations such as “let h be heat transfer, m mass, …” lost later role evidence                      | One bounded clause grammar maps arbitrary-arity copula elision without pack-specific phrases                                                                              |
| Typed role shape            | A strongly typed quantity could be rejected only because no redundant scalar word was present                          | Quantity evidence admits an absent shape while explicit incompatible shape evidence still refuses the law                                                                 |
| Section scope               | Markdown sibling headings could leak acronym and prose evidence                                                        | wasmtex emits nested Markdown scopes; Semath resolves evidence only inside the structural scope graph                                                                     |
| Equation equivalence        | Solved and coefficient variants were ad-hoc unguarded law rewrites                                                     | Schema 8 provides one canonical relation; a bounded typed compiler emits guarded derived forms and proof evidence without commuting matrix/operator products              |
| Structural token identity   | Semath inferred token kinds again from source text, splitting decorated and named notation                             | wasmtex syntax 7 supplies neutral lexical classes and document fields; Semath consumes them without TeX re-lexing or interpreting domains                                 |
| Domain routing              | Global weak/strong activations could neither express scoped relevance nor order cross-pack collisions                  | Catalog-derived signatures and collision reports feed explicit/supported/tentative hypotheses; relevance orders a bounded frontier without becoming proof or a diagnostic |
| Formula/prose order         | Typed descriptions after an equation arrived too late to support that equation                                         | One bounded math-slot construction attaches formula-first and prose-first declarations with exact evidence and deterministic retraction                                   |
| Equation references         | A later “In Equation …” role list could not support the labeled display equation                                       | Label/reference attachment links the two exact ranges; equation metadata is removed from canonical meaning                                                                |
| Scientific operators        | Gradient, divergence, curl, Laplacian, conditional arguments, and callable role placeholders lost reusable structure   | One shared canonical IR and generic unifier represent them without field-specific runtime branches                                                                        |
| Decorated and indexed roles | Decorated optima and indexed state families lost identity or role evidence                                             | Canonical decorated atoms and indexed-family lookup preserve exact source notation while sharing declared family evidence                                                 |
| Opaque expansions           | Macro resolution status conflated ordinary commands with truly opaque generated notation                               | Only opaque/cyclic/truncated structure yields an engine-limit decision; no Problems diagnostic is invented                                                                |
| Canonical relations         | Labels, chained equalities, and comma-separated systems could distort the relation extent or lose reusable constraints | Exact equation ranges and one canonical system path now preserve relation heads and all source-linked constraints                                                         |
| Scoped comparisons          | A rendered spelling could accidentally stand in for semantic identity                                                  | Comparison and retraction authority now follows resolved entities, source order, scopes, and connected components                                                         |
| Document conditions         | Included roles propagated but their typed assumptions stopped at the file boundary                                     | The existing ordered external type environment now carries subject-bound assumptions and retracts them through the same dependency closure                                |
| Prose composition           | Result actions, role-first clauses, and formula-following `where` or `Here` assumptions used separate narrow branches  | The normalized event stream composes these bounded forms and rejects cited, hedged, hypothetical, negated, or contradictory evidence                                      |
| Characterized operators     | An existentially characterized result and a later typed operator assignment remained disconnected                    | Bounded nominal shapes and unambiguous demonstratives retain the result role; the typed operator plan proves the matching domain condition from the same evidence          |
| Law runtime                 | A legacy single-result unifier coexisted with bounded multi-result matching                                            | One bounded unifier remains, preserving structural recognition while deleting the obsolete path                                                                           |
| Approximation semantics     | Approximation notation could collapse into exact equality and inherit unsupported proof authority                     | A distinct directional comparison preserves source evidence without entering equality closure; asymptotic order remains bound-class membership                            |

## Remaining measured gaps

| Category            | Current limitation                                                                                                   | Affected evidence                                                                                                    |
| ------------------- | -------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Coverage            | Evaluated vertical slices do not yet provide broad field recognition                                                 | Electromagnetism, thermodynamics/heat transfer, fluid mechanics, calculus, discrete mathematics, and optimization/ML |
| Local recognition   | Existing prose events, typed facts, and pack unification do not yet retain enough evidence from realistic exposition | All 58 current development misses localize to attachment (21), typed facts (19), or pack unification (18)            |

The remaining limitations are inputs to later roadmap issues. They require
shared primitives and measured evidence; they must not be hidden by
pack-specific matchers or lower thresholds.
