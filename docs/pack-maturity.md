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
| Foundation evidence | Quantity/unit support was judged by a zero-law count | Added a strict 40-case non-law corpus and pure scorer |

## Remaining measured gaps

| Category | Current limitation | Affected evidence |
| --- | --- | --- |
| Generic IR | Set union and intersection match canonically but are not yet first-class relation operators with membership/result typing | Probability and discrete mathematics |
| Generic IR | Function application, composition, indexed families, binders, partial derivatives, gradients, and integrals lack one complete compositional typing path | Calculus, optimization, linear algebra, and control systems |
| Concept identity | Law-role prose is discovered from pack data, but runtime role claims still use unnamespaced suffixes that can collide as the catalog grows | Future packs with shared role names |
| Constraints | Side conditions are reported but not uniformly represented as reusable typed constraints over independent variables, domains, shapes, and assumptions | Matrix products, derivatives, optimization updates, and engineering laws |
| Project environment | Exported project facts do not yet propagate quantity/unit declarations through includes with the same contract as local facts | Quantities/units and any quantity-bearing field pack |
| Macro semantics | Macros used inside prose declarations do not yet contribute quantity facts; opaque expansion must continue to refuse | Quantities/units and future notation-heavy packs |
| Coverage | Probe packs demonstrate runtime viability, not broad field recognition | Calculus, discrete mathematics, and optimization/ML |

The first six limitations are concrete inputs to
[Roadmap 3/7](https://github.com/corca-ai/semath/issues/149). They require
domain-neutral primitives shared by multiple packs; they must not be hidden by
pack-specific matchers or lower thresholds.
