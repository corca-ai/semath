# Public API

Semath is a library and language-service runtime. Protocol 14 is a deliberate
hard cutover to a small meaning-first API.

| Export | Responsibility |
| --- | --- |
| `semath/evaluation` | pure corpus validation, metamorphic planning, scoring, and pack conformance |
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
unsupported `decision` as a discriminated union. Established and partial
decisions contain a structured known meaning; partial decisions may also carry
neutral requirements for optional tooling. Ambiguous decisions contain bounded
alternatives, and conflicting decisions contain source-linked conflicts. Every
state has a deterministic bounded reason slice whose kinds distinguish proof,
uncertainty, engine limits, and demonstrated source conflicts. A host must not
turn uncertainty or an engine limit into a document diagnostic. Required or
unsupported law conditions and truncated evidence cannot produce `established`.
`context.assumptions` contains explicit, source-linked assumptions with their
subjects; omission means none were established. Parser ASTs, free-form refusal
policy, and legacy rewrite queries are not public.

Protocol 14 identifies every `RoleInfo` by its open, pack-qualified `conceptId`.
There is no closed role enum or unnamespaced compatibility field. Included-file
role, shape, and quantity facts use the same records and retain their original
evidence.

Every `LawBinding` exposes a typed `proof` disposition. `typed` and `derived`
bindings may participate in an established decision. `asserted` preserves a
source-backed formula-level proposal without inventing the role identity, and
`candidate` remains unresolved. Hosts display this state; they do not infer it
from evidence labels or strength strings.

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

Core semantic behavior belongs to Rust. Packages own transport and lifecycle;
applications own presentation, permissions, review, apply, and undo.
