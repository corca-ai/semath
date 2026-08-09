# Pack maturity report

This is measured repository state on 2026-08-09, not live production telemetry
and not a future plan. The [quality manifest](../fixtures/corpus-manifest.json)
holds approved support policy; GitHub issues hold planned work.

## Evidence

- 1,890 authored law cases cover 17 laws in nine packs, including 48 positive
  and 32 refusal diversity cases for every promoted law.
- 157 deterministic metamorphic cases preserve results under irrelevant prose,
  comments, and document ordering.
- All law families currently score 100% for recall, precision, role binding,
  source-linked evidence, and refusal preservation.
- The quantities/units foundation passes 46 of 46 non-law cases for quantity,
  unit, dimension, propagation diagnostics, notation, prose, and role evidence.
- The shared scientific-kernel foundation passes 16 of 16 positive and refusal
  cases across explicit operators, typed application, namespaced concepts,
  include order, and transparent or opaque prose macros.
- Linear algebra and probability meet evaluated policy. Calculus, discrete
  mathematics, and optimization have useful probe families but do not claim
  field-wide coverage from one or two law families.

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
| Concept identity | Runtime roles could collide as unnamespaced suffixes | Protocol 4 and the core use pack-qualified concept identities without a legacy role field |
| Project environment | Included facts only affected law matching and dropped rich quantity facts | One ordered external type environment now exports complete role, shape, quantity, unit, dimension, and evidence records |
| Macro semantics | A macro command name could be mistaken for prose meaning | Only wasmtex-approved transparent surfaces contribute prose quantity meaning; opaque calls refuse |

## Remaining measured gaps

| Category | Current limitation | Affected evidence |
| --- | --- | --- |
| Generic IR | Indexed families, partial derivatives, gradients, integrals, and operator result typing do not yet share a complete constraint path | Calculus, optimization, linear algebra, and control systems |
| Constraints | Side conditions are reported but not uniformly represented as reusable typed constraints over independent variables, domains, shapes, and assumptions | Matrix products, derivatives, optimization updates, and engineering laws |
| Coverage | Probe packs demonstrate runtime viability, not broad field recognition | Calculus, discrete mathematics, and optimization/ML |

The remaining limitations are inputs to later roadmap issues. They require
shared primitives and measured evidence; they must not be hidden by
pack-specific matchers or lower thresholds.
