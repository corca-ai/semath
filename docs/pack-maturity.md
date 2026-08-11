# Pack maturity report

This is measured repository state on 2026-08-12, not live production telemetry
and not a future plan. The [quality manifest](../fixtures/corpus-manifest.json)
holds approved support policy; GitHub issues hold planned work.

## Evidence

- 5,770 scored law cases cover 61 laws in twelve formula packs, including 48
  positive and 32 refusal diversity cases for every promoted law. Of all 5,826
  scored corpus cases, 420 remain readable fixture seeds and 5,406 are
  deterministic cases reproduced in memory from compact specs, packs, and
  seeds; generated cases are not called independently authored evidence.
- 453 deterministic metamorphic cases preserve results under irrelevant prose,
  comments, and document ordering.
- All law families currently score 100% for recall, role binding, and
  source-linked evidence. The manual v0.27 baseline retains four known
  precision/refusal failures for `event-intersection`; the release must resolve
  them without lowering thresholds.
- The quantities/units foundation passes 52 of 52 non-law cases for quantity,
  unit, dimension, propagation diagnostics, notation, prose, and role evidence.
- The shared scientific-kernel and scientific-prose foundations pass 166 of 166
  positive and refusal cases across explicit operators, typed application,
  namespaced concepts, include order, and transparent or opaque prose macros.
- Every formula pack meets evaluated policy for its declared vertical. The six
  previously narrow fields now add measured constitutive/wave, heat-transfer,
  transport, nabla, graph/counting, and objective/constraint clusters. These
  are useful verticals, not claims of field completeness.
- The independent frozen challenge v3 preserves 48 semantic boundaries across
  binding, constraints, packs, presentation, resolution, and syntax, then
  composes them into six realistic document shapes. The v0.27 baseline passes
  33 of 48; all 15 misses are unsafe meaning establishment in cases that should
  remain partial without meaning. Full execution remains a manual release gate,
  and the release must remove these regressions without changing the fixture.
- The guarded-equivalence challenge freezes 24 separately authored orientation,
  scalar permutation, isolation, reciprocal, grouping, and noncommutative
  refusal cases. The pre-change `a978d2e` baseline passed 21; the current engine
  passes 24. Schema and coverage checks remain fast CI tests, while full engine
  execution is a manual release gate.
- The scoped-domain challenge freezes 30 independently authored document cases
  covering neutral fields, section scope, mixed fields, non-evidence,
  formula-before/after attachment, ambiguity, conflict, and retraction. Protocol
  10 could not represent tiered domain relevance or relevance-ordered
  alternatives; protocol 12 passes all 30 cases. Six cases independently cover
  every current structural collision component with typed formula contexts.
- The recognition-frontier v1 fixture freezes 32 stage-labeled cases across
  eight notation, discourse, lifecycle, safety, mathematics, and engineering
  families. The current engine passes 32 of 32 with zero false establishment,
  false conflict, or missed-coverage risk under the reviewed targets.
- The v0.27 authored document baseline was run manually against Semath commit
  `0380421` after its 96 development scenarios were reviewed and its 48 holdout
  scenarios were separately reviewed and frozen before engine execution.
  Development passes 7 of 115 probes
  with risk 484; the now-exposed historical holdout passes 4 of 97 with risk
  732. Both splits have zero false establishment and zero false conflict. The
  failures localize as follows:

  | First loss | Development | Holdout |
  | --- | ---: | ---: |
  | Neutral syntax | 0 | 0 |
  | Prose attachment | 28 | 18 |
  | Identity or scope | 30 | 49 |
  | Canonical IR | 11 | 1 |
  | Typed fact or condition | 6 | 0 |
  | Local-to-observation propagation | 0 | 0 |
  | Pack unification | 29 | 24 |
  | Decision | 4 | 1 |
  | Host projection | Not exercised | Not exercised |

  The baseline reuses the existing recognition-frontier signals and queries
  each reviewed relation at its original equation. A zero propagation count
  therefore means no failing expected relation survived local recognition; it
  does not prove that propagation is complete. The measured order of work is
  identity/scope, local pack and prose recognition, then canonical or typed
  gaps. Host parity remains release evidence rather than a native-baseline
  inference.

These numbers describe the synthetic benchmark, not real-world prevalence or
field completeness.

## Gaps resolved in this evaluation

| Category | Finding | Resolution |
| --- | --- | --- |
| Pack data | Linear algebra lacked matrix product/transpose; probability lacked union; three vocabulary packs had no measured laws | Added typed pack laws and independently seeded positive/refusal families |
| Generic prose | Common plural declarations such as “events,” “sets,” and “iterates” lost roles | Law-role vocabulary now derives from pack concepts and handles singular/plural wording |
| Generic prose | “In this setting, let X and Y …” lost the first coordinated declaration | Coordinated declarations now accept an introductory clause without a pack branch |
| Fixture quality | Several seeds used invalid TeX words such as `alpha` or semantically invalid “respectively” phrasing | Corrected authored seeds; generation and duplicate/integrity gates prevent silent drift |
| Maturity policy | One pack-wide tier conflated vocabulary, laws, typing, and provenance | Manifest schema 3 declares all seven capabilities separately and derives the summary |
| Foundation evidence | Quantity/unit support was judged by a zero-law count | Added a strict 46-case non-law corpus and pure scorer |
| Generic IR | Union/intersection were implicit and function calls were narrowly recovered | Added explicit set/relation operators, ordered composition, and multi-argument application on one canonical path |
| Concept identity | Runtime roles could collide as unnamespaced suffixes | Protocol 12 and the core use pack-qualified concept identities without a legacy role field |
| Project environment | Included facts only affected law matching and dropped rich quantity facts | One ordered external type environment now exports complete role, shape, quantity, unit, dimension, and evidence records |
| Macro semantics | A macro command name could be mistaken for prose meaning | Only wasmtex-approved transparent surfaces contribute prose quantity meaning; opaque calls refuse |
| Engineering composition | Shared notation could activate an unrelated field | Added typed role/quantity composition and a 16-case cross-field refusal suite |
| Pack vocabulary | Natural concept paraphrases required runtime vocabulary edits | Schema 8 retains reviewed concept aliases consumed by the generic classifier |
| Constraints | Side conditions were free-form strings without machine-checkable subjects or evidence | Schema 8 uses closed constraint kinds, validated law roles, bound symbols, source evidence, and explicit resolution status |
| Generic calculus IR | Integrals, partial derivatives, nabla applications, and indexed families lost operator structure | The shared structural path now keeps explicit differential variables, derivative order, integral bounds, and base/index components without command-specific pack logic |
| Source notation | Styled, decorated, and declared operator surfaces collapsed to a leaf or failed at a trailing application edge | Exact wasmtex identity ranges and precomputed complete-application boundaries preserve source notation without retaining the syntax arena at query time |
| Prose declarations | Elided parallel declarations such as “let h be heat transfer, m mass, …” lost later role evidence | One bounded clause grammar maps arbitrary-arity copula elision without pack-specific phrases |
| Typed role shape | A strongly typed quantity could be rejected only because no redundant scalar word was present | Quantity evidence admits an absent shape while explicit incompatible shape evidence still refuses the law |
| Section scope | Markdown sibling headings could leak acronym and prose evidence | wasmtex emits nested Markdown scopes; Semath resolves evidence only inside the structural scope graph |
| Equation equivalence | Solved and coefficient variants were ad-hoc unguarded law rewrites | Schema 8 provides one canonical relation; a bounded typed compiler emits guarded derived forms and proof evidence without commuting matrix/operator products |
| Structural token identity | Semath inferred token kinds again from source text, splitting decorated and named notation | wasmtex syntax 7 supplies neutral lexical classes and document fields; Semath consumes them without TeX re-lexing or interpreting domains |
| Domain routing | Global weak/strong activations could neither express scoped relevance nor order cross-pack collisions | Catalog-derived signatures and collision reports feed explicit/supported/tentative hypotheses; relevance orders a bounded frontier without becoming proof or a diagnostic |
| Formula/prose order | Typed descriptions after an equation arrived too late to support that equation | One bounded math-slot construction attaches formula-first and prose-first declarations with exact evidence and deterministic retraction |
| Equation references | A later “In Equation …” role list could not support the labeled display equation | Label/reference attachment links the two exact ranges; equation metadata is removed from canonical meaning |
| Scientific operators | Gradient, divergence, curl, Laplacian, conditional arguments, and callable role placeholders lost reusable structure | One shared canonical IR and generic unifier represent them without field-specific runtime branches |
| Decorated and indexed roles | Decorated optima and indexed state families lost identity or role evidence | Canonical decorated atoms and indexed-family lookup preserve exact source notation while sharing declared family evidence |
| Opaque expansions | Macro resolution status conflated ordinary commands with truly opaque generated notation | Only opaque/cyclic/truncated structure yields an engine-limit decision; no Problems diagnostic is invented |

## Remaining measured gaps

| Category | Current limitation | Affected evidence |
| --- | --- | --- |
| Coverage | Evaluated vertical slices do not yet provide broad field recognition | Electromagnetism, thermodynamics/heat transfer, fluid mechanics, calculus, discrete mathematics, and optimization/ML |
| Document identity | Realistic symbol boundaries, declarations, edits, and scoped reuse frequently lose the intended entity | 79 of 201 failed authored probes localize first to identity or scope |
| Local recognition | Existing prose events and pack unification do not yet retain enough evidence from realistic exposition | 105 of 201 failed authored probes localize to attachment, typed facts, or pack unification |
| Canonical structure | A smaller set of reviewed formulas loses reusable structure before matching | 12 of 201 failed authored probes localize to canonical IR |

The remaining limitations are inputs to later roadmap issues. They require
shared primitives and measured evidence; they must not be hidden by
pack-specific matchers or lower thresholds.
