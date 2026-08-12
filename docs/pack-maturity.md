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
- All 61 evaluated laws score 100% for recall, precision, role binding,
  source-linked evidence, and refusal preservation across 5,826 cases. The 453
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
  alternatives; protocol 12 passes all 30 cases. Six cases independently cover
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
  probes with risk 732. The current release passes 17 development probes with
  risk 404 and 6 historical holdout probes with risk 720. Development has no
  false establishment or false conflict. The holdout reports one raw false
  establishment because its stage-6 expectation asks prose in a disconnected
  file to retract an established relation in another component; preserving the
  component boundary is the safer engine behavior, so the frozen expectation is
  retained and documented rather than matched by global string retraction. The
  current first losses localize as follows:

  | First loss | Development | Holdout |
  | --- | ---: | ---: |
  | Neutral syntax | 0 | 0 |
  | Prose attachment | 26 | 19 |
  | Identity or scope | 26 | 48 |
  | Canonical IR | 0 | 0 |
  | Typed fact or condition | 9 | 3 |
  | Local-to-observation propagation | 1 | 0 |
  | Pack unification | 21 | 18 |
  | Decision | 15 | 3 |
  | Host projection | Not exercised | Not exercised |

  The scorer reuses the existing recognition-frontier signals and queries
  each reviewed relation at its original equation. A low propagation count
  does not by itself prove that propagation is complete. The one development
  propagation expectation asks an unrelated earlier relation to appear at a
  later equation with no shared entity or derivation edge; Semath safely keeps
  it local. The measured order of work is
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
| Canonical relations | Labels, chained equalities, and comma-separated systems could distort the relation extent or lose reusable constraints | Exact equation ranges and one canonical system path now preserve relation heads and all source-linked constraints |
| Scoped comparisons | A rendered spelling could accidentally stand in for semantic identity | Comparison and retraction authority now follows resolved entities, source order, scopes, and connected components |
| Document conditions | Included roles propagated but their typed assumptions stopped at the file boundary | The existing ordered external type environment now carries subject-bound assumptions and retracts them through the same dependency closure |
| Prose composition | Result actions, role-first clauses, and formula-following `where` or `Here` assumptions used separate narrow branches | The normalized event stream composes these bounded forms and rejects cited, hedged, hypothetical, negated, or contradictory evidence |
| Law runtime | A legacy single-result unifier coexisted with bounded multi-result matching | One bounded unifier remains, preserving structural recognition while deleting the obsolete path |

## Remaining measured gaps

| Category | Current limitation | Affected evidence |
| --- | --- | --- |
| Coverage | Evaluated vertical slices do not yet provide broad field recognition | Electromagnetism, thermodynamics/heat transfer, fluid mechanics, calculus, discrete mathematics, and optimization/ML |
| Document identity | Realistic declarations, edits, scoped reuse, and navigation projections still frequently lose the intended entity | 74 of 189 failed authored probes localize first to identity or scope |
| Local recognition | Existing prose events, typed facts, and pack unification do not yet retain enough evidence from realistic exposition | 96 of 189 failed authored probes localize to attachment, typed facts, or pack unification |
| Decision projection | Available evidence still produces the wrong calm/established distinction in some realistic scenes | 18 of 189 failed authored probes localize first to the decision boundary |

The remaining limitations are inputs to later roadmap issues. They require
shared primitives and measured evidence; they must not be hidden by
pack-specific matchers or lower thresholds.
