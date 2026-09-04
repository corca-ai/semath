# Public API

Semath is a conservative mathematical document analyzer and language-service
runtime using protocol 18. The [supported scope](conservative-analysis.md)
defines product guarantees and the authority of each result.
An established symbol identifies a source-defined entity. An established
formula requires grounded typed roles and verified conditions; neither a
source description nor a prose verdict proves an equation.

| Export | Responsibility |
| --- | --- |
| `semath/authoring` | pack compiler report types and validated packaging |
| `semath/protocol` | snapshots, deltas, semantic queries, results, and diagnostics |
| `semath/wasm` | release WASM and its byte-oriented engine ABI |
| `semath/worker` | typed WASM engine wrapper |
| `semath/worker-host` | scheduling, recovery, and stale-generation fencing |
| `semath/worker-runtime` | Worker-side request dispatch |
| `semath/wasmtex-adapter` | pure wasmtex-to-Semath document conversion |
| `semath/lsp` | standard navigation plus `semath/semanticView` |

Hosts send a complete `ProjectSnapshot`, then ordered `ChangeEnvelope` deltas.
Every request carries protocol, inventory, document, and analysis versions so
stale results can be rejected.
An upsert may repeat a document version only for a wasmtex structural relink of
identical source and path. Semath requires a changed structural fingerprint;
same-version text changes remain stale and are ignored.

The query surface is:

- `selection`
- `semanticView`
- `definition` and `references`
- `prepareRename` and `rename`
- `diagnostics` and `explainDiagnostic`

`semanticView` returns an established, partial, ambiguous, conflicting, or
unsupported cursor-entity `decision` as a discriminated union. Established and
partial decisions contain a structured known meaning; partial decisions may
also carry neutral requirements for optional tooling. Ambiguous decisions
contain bounded alternatives, and conflicting decisions contain source-linked
conflicts. The selected formula has its own independently derived disposition
in `semanticView.formulaAnalysis`: evidence about the complete formula cannot
establish or conflict the entity at an interior cursor, and an independently
established entity cannot establish the surrounding formula. Every state has a
deterministic bounded reason slice whose kinds distinguish proof, uncertainty,
engine limits, and demonstrated source conflicts. A host must not turn
uncertainty or an engine limit into a document diagnostic. Required or
unsupported law conditions and truncated evidence cannot produce `established`.
`context.assumptions` contains explicit, source-linked assumptions with their
subjects; omission means none were established. Parser ASTs, free-form refusal
policy, and legacy rewrite queries are not public.

Protocol 15 identifies every `RoleInfo` by its open, pack-qualified `conceptId`.
There is no closed role enum or unnamespaced compatibility field. Included-file
role, shape, and quantity facts use the same records and retain their original
evidence.

Every `LawBinding` exposes a typed `proof` disposition. `typed` and `derived`
bindings may participate in an established decision. `asserted` preserves a
source-backed formula-level proposal without inventing the role identity, and
`candidate` remains unresolved. Hosts display this state; they do not infer it
from evidence labels or strength strings.

The binding `constraint` retains its closed shape kind and observed symbolic
extents when they are known. A `LawConditionInfo` keeps bound source subjects
separate from its display label and may expose a closed `operatorProperty` for
an `operator-property` condition. `maps-between` and `rank-compatible` likewise
use ordered bound subjects. These additive protocol-14 values let hosts explain
missing evidence without parsing English labels or doing mathematical
inference.

Source selection exposes a revision-local `SourceOccurrenceId`; established
meaning exposes a scoped `EntityId` anchored to one such occurrence. Notation
components such as modifiers, styles, scripts, and named operators remain part
of the occurrence and never become flat string identities. Definitions and
references resolve through the project semantic index, not a project-wide
symbol scan.

Every definition, references, prepare-rename, and rename result carries an
`authorization`. Authorized results expose the exact focus `SourceOccurrenceId`
and resolved `EntityId`; refused results expose a typed reason. An authorized
empty definition (for example, querying an entity at its own declaration) is
therefore distinct from unsupported, ambiguous, conflicting, engine-limited,
incomplete, non-editable, invalid-replacement, and capture refusals. Hosts must
not fall back to another symbol or rename index after a refusal. References can
include or exclude the source declaration explicitly, and rename never returns a
partial edit set.

Navigation and meaning decisions answer different questions. Definition and
references are available only when the exact cursor occurrence resolves to one
source-grounded entity; definition returns a location only when that entity has
a real source declaration. Navigation may remain available when
the surrounding formula is partial or unsupported. Conversely, an established
formula does not establish the identity of every symbol inside it. References
are complete for that entity or unavailable—hosts must not merge a fallback
symbol scan into an empty semantic result.
Formula-scoped meaning facts follow the same rule: they may establish the exact
queried formula occurrence while definition, references, and rename remain
refused because no declaration entity was created.
When a nested occurrence shares the canonical relation's terminal boundary,
the relation fact applies only to a query at that exact boundary. Moving the
cursor inside the nested occurrence does not inherit the relation's proof.

The authoritative declaration is the exact occurrence named by an asserted,
positive, explicit `Defines` claim in the project semantic index. Presentation
metadata may describe that declaration but cannot authorize navigation or edits.

Rename has a stricter contract than navigation. `prepareRename` and `rename`
must agree on the same established entity and complete occurrence set. Every
edit must target real source, preserve one notation family, avoid capture or an
entity merge, and fit the bounded fan-out limit. Generated macro text, a base
inside an indexed or decorated occurrence, mixed aliases, ambiguous scope, or
an incomplete result makes the whole operation unavailable; Semath never emits
a partial edit.

Selection identity is lowered before the syntax arena is compacted. Composite
notation retains its exact identity range, and callable occurrences retain only
the exact end of a complete following argument. Cursor queries therefore do
not rescan TeX, retain the frontend tree, or snap a callable across unrelated
trailing syntax.

Project documents contain the complete wasmtex syntax schema 8 snapshot, including
neutral lexical classes, document fields, and citation annotations alongside visible prose and
structural scopes. The
adapter validates the schema and forwards the arena, roots, visible prose,
scopes, declarations, and provenance without reconstructing or selectively
copying structural facts. Corrupt top-level contracts fail explicitly;
incomplete or opaque subtrees remain local unsupported evidence.

`semanticView.domains` reports bounded scoped hypotheses with `explicit`,
`supported`, or `tentative` support and exact source evidence. Formula and
meaning alternatives may carry the same relevance projection. Relevance orders
alternatives but never changes a decision from unsupported to established.
`AnalysisStats` exposes domain evidence/hypothesis and frontier/latent work so
hosts and release gates can detect routing regressions without reimplementing
policy.

`semanticView.formulaAnalysis` is always present on a current successful
semantic query. Its `disposition` is the independently computed state of the
selected complete formula: established, partial, ambiguous,
conflicting, unsupported, or engine-limited. Its optional `formula` supplies
the exact file, path, revision, scope, UTF-16 range, authored source notation,
and generated provenance for the complete math region. `lifecycle` reports only
Semath-owned source facts:
authored versus generated, current freshness, retraction, editability of the
source surface, capping, and engine limits. A stale query returns the existing
typed query error rather than stale formula analysis.

The formula analysis also exposes structured missing role declarations and
conditions, bounded exact same-entity notation occurrences, prose claim anchors
with polarity, modality, and evidence parents, and a
distinct approximation disposition for canonical approximate relations.
Equation links distinguish `shared-entity` continuity from `derived-law`
evidence; only the latter can ground consequence wording. All collections are
source ordered, capped, retractable, and reproduced by native, WASM, Worker,
and LSP boundaries.

`FormulaAnalysisInfo` contains no phrase ID, rhetorical move, English
template, UI category, recommendation score, or conclusion that prose should
be inserted. Hosts remain responsible for writing policy, permissions,
preview, mutation, collaboration, and undo.

### Evidence-graded interpretations

Protocol 18 exposes `formulaAnalysis.interpretations` as a bounded, open-world
projection over the existing meaning decision, typed laws, scoped domain
hypotheses and structural alternatives. It does not replace or weaken the
meaning decision. A host may present the hypotheses for manual review, but the
projection grants no diagnostic, navigation, rename, or edit authority.

Each hypothesis preserves a stable candidate identity, qualitative support
tier, exact current file/path/range, document revision, scope path, optional
formula anchor, typed relation, bindings and conditions when available, and
the requirement IDs that would discriminate it. `evidence` classifies every
source-linked item independently as supporting or contradicting and records
whether it came from an explicit declaration, typed structure, natural-language
extraction, scoped domain context, or derived evidence.
Each item retains its underlying `Evidence` and adds `sourceAnchors`; every
anchor identifies the evidence's own file, path, range, document revision,
scope path, current or retracted lifecycle, and authored or generated origin.
Cross-document evidence is therefore never labeled as if it came from the
cursor document.
These anchors travel with evidence inside the semantic core before projection;
Semath does not guess a document by matching a rule ID and numeric range.
These dimensions are deliberately inspectable; Semath exposes no confidence
percentage or opaque weighted score.

`orderingReasons` explains deterministic precedence through closed reason
kinds and exact evidence. Multiple plausible interpretations remain in the
ordered set. `missingDiscriminators` contains the complete typed requirement
objects referenced by hypotheses rather than presentation strings that a host
would have to parse. Every evidence-bearing requirement condition, alternative,
alternative relevance, ordering reason, and analysis limit uses the same exact
evidence-reference envelope. The top-level `requirements` collection contains
that projected type too; raw numeric ranges are never the public identity of
interpretation evidence.

The set always reports `bounded-open-world` exhaustiveness: absence from the
bounded candidates does not mean that an author's intended interpretation is
impossible. `analysisLimits` separately reports candidate/evidence capping,
discriminator capping, engine limits, generated source, and retraction. Caps in
unrelated formula-analysis views do not mark the interpretation set truncated.
A missing hypothesis or
advisory facet therefore suppresses only the dependent host assistance and
never becomes a Problem.

`scoped-domain` and `structural-alternative` hypotheses are advisory. Even an
explicit domain activation cannot give either kind `explicit` or `derived`
mathematical authority. A structural alternative has a typed, source-anchored
disambiguation requirement; it is not silently presented as a scientific
meaning merely because the parser can represent it.

Core semantic behavior belongs to Rust. Packages own transport and lifecycle;
applications own presentation, permissions, review, apply, and undo.
