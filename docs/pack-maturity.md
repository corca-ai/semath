# Pack maturity report

This is measured repository state on 2026-08-11, not live production telemetry
and not a future plan. The [quality manifest](../fixtures/corpus-manifest.json)
holds approved support policy; GitHub issues hold planned work.

## Evidence

- 4,146 authored law cases cover 45 laws in twelve packs, including 48 positive
  and 32 refusal diversity cases for every promoted law.
- 327 deterministic metamorphic cases preserve results under irrelevant prose,
  comments, and document ordering.
- All law families currently score 100% for recall, precision, role binding,
  source-linked evidence, and refusal preservation.
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
  composes them into six realistic document shapes. The engine passes 48 of 48,
  including all five decision states, source-grounded explanations, and the
  neutral-versus-conflict Problems policy. Full execution remains a manual
  release gate.
- The guarded-equivalence challenge freezes 24 separately authored orientation,
  scalar permutation, isolation, reciprocal, grouping, and noncommutative
  refusal cases. The pre-change `a978d2e` baseline passed 21; the current engine
  passes 24. Schema and coverage checks remain fast CI tests, while full engine
  execution is a manual release gate.

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
| Concept identity | Runtime roles could collide as unnamespaced suffixes | Protocol 10 and the core use pack-qualified concept identities without a legacy role field |
| Project environment | Included facts only affected law matching and dropped rich quantity facts | One ordered external type environment now exports complete role, shape, quantity, unit, dimension, and evidence records |
| Macro semantics | A macro command name could be mistaken for prose meaning | Only wasmtex-approved transparent surfaces contribute prose quantity meaning; opaque calls refuse |
| Engineering composition | Shared notation could activate an unrelated field | Added typed role/quantity composition and a 16-case cross-field refusal suite |
| Pack vocabulary | Natural concept paraphrases required runtime vocabulary edits | Schema 8 retains reviewed concept aliases consumed by the generic classifier |
| Constraints | Side conditions were free-form strings without machine-checkable subjects or evidence | Schema 6 uses closed constraint kinds, validated law roles, bound symbols, source evidence, and explicit resolution status |
| Generic calculus IR | Integrals, partial derivatives, nabla applications, and indexed families lost operator structure | The shared structural path now keeps explicit differential variables, derivative order, integral bounds, and base/index components without command-specific pack logic |
| Source notation | Styled, decorated, and declared operator surfaces collapsed to a leaf or failed at a trailing application edge | Exact wasmtex identity ranges and precomputed complete-application boundaries preserve source notation without retaining the syntax arena at query time |
| Prose declarations | Elided parallel declarations such as “let h be heat transfer, m mass, …” lost later role evidence | One bounded clause grammar maps arbitrary-arity copula elision without pack-specific phrases |
| Typed role shape | A strongly typed quantity could be rejected only because no redundant scalar word was present | Quantity evidence admits an absent shape while explicit incompatible shape evidence still refuses the law |
| Section scope | Markdown sibling headings could leak acronym and prose evidence | wasmtex emits nested Markdown scopes; Semath resolves evidence only inside the structural scope graph |
| Equation equivalence | Solved and coefficient variants were ad-hoc unguarded law rewrites | Schema 8 provides one canonical relation; a bounded typed compiler emits guarded derived forms and proof evidence without commuting matrix/operator products |
| Structural token identity | Semath inferred token kinds again from source text, splitting decorated and named notation | wasmtex syntax 6 supplies neutral lexical classes for source and expanded notation; Semath consumes them without TeX re-lexing |

## Remaining measured gaps

| Category | Current limitation | Affected evidence |
| --- | --- | --- |
| Coverage | Evaluated vertical slices do not yet provide broad field recognition | Electromagnetism, thermodynamics/heat transfer, fluid mechanics, calculus, discrete mathematics, and optimization/ML |

The remaining limitations are inputs to later roadmap issues. They require
shared primitives and measured evidence; they must not be hidden by
pack-specific matchers or lower thresholds.
