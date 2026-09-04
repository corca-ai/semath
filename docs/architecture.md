# Architecture

Semath implements [conservative mathematical document analysis](conservative-analysis.md)
in a host-independent Rust core. CorTeX and other hosts own editing and presentation.

## Data flow

```text
project snapshot or delta
  → wasmtex structural syntax and source provenance
  → bindings, scopes, and ProjectSemanticIndex
  → canonical expressions and grounded typed constraints
  → source-linked views, navigation, diagnostics, and reviewed edits
  → native, WASM, Worker, and LSP boundaries
```

wasmtex owns syntax. Semath accepts its syntax contract without rescanning raw
TeX to build a competing model. Native and WASM use the same Rust transformations.
Packages own transport and lifecycle, with no second semantic implementation.

## Identity and evidence

`ProjectSemanticIndex` is the project-wide authority for source occurrences,
scoped entities, claims, evidence, candidates, and dependencies. Occurrence IDs
include the source revision. Entity IDs are anchored to a scoped occurrence;
spelling is not identity. Aliases respect scope and source order and retract
with their evidence.

Each analyzed document retains compact immutable observations after releasing
the full syntax tree. Queries resolve those observations through the project
index. No parallel project fact store or presentation map authorizes edits.

Claims retain polarity, modality, extraction rule, provenance, and bounded
derivation tiers. Their values form a closed algebra of types, shapes,
dimensions, units, conditions, relations, and literals. Presentation strings
never become semantic facts.

Scope controls visibility; attachment controls which source blocks can exchange
evidence; domain routing orders compatible typed rules. These are separate
transformations. None may substitute for another's authority.

## Conservative inference

Explicit declarations and supported typed operations establish constraints.
The bounded English declaration grammar is an input adapter, not a general
language-understanding system. Questions, hedges, negation, citations, and
unattached prose cannot become affirmative source facts.

A symbol definition identifies the symbol. It does not prove an enclosing
formula. Formula establishment requires independently grounded role bindings
and verified conditions. Conflicts require incompatible grounded facts or an
invalid typed operation. Unsupported input and exhausted bounds are analysis
limits, not document errors.

Structural ambiguity comes from actual syntax. Compatible typed interpretations
retain their evidence and missing conditions. Domain relevance orders them but
cannot create a meaning from conventional notation or resolve a genuine collision.

[Domain packs](domain-packs.md) compile once into immutable dispatch indexes.
All typed laws use the same generic unifier, with capped equivalent forms and
condition checks. Pack IDs never select runtime behavior. Established relations
may supply consumed role evidence to later formulas for at most two forward
hops and 64 roles per target. Those derivations retain relation parents and use
the same retraction mechanism as other claims.

## Queries and edits

[Public queries](public-api.md) carry project, document, and analysis revisions.
Stale queries return typed errors. Definition, references, and rename share one
entity authorization policy. Rename requires a complete editable reference set
and rejects capture, generated ranges, ambiguity, or exceeded bounds as a whole.

Formula analysis describes the complete selected syntax root. Moving the cursor
between its children must not change the formula's decision. Symbol-owned
facts and formula-owned facts retain separate authority.

Worker generations and reset transactions prevent partially updated state from
becoming visible. A semantic edit rebuilds only the affected dependency closure;
comment-only edits preserve semantic state. Removing evidence retracts every
dependent result. Clean and incremental analysis must agree.

## Verification boundaries

- Rust tests own inference, scope, identity, evidence, and refusal permutations.
- Real Markdown/TeX acceptance checks verify useful behavior and abstention.
- Native/WASM parity and lifecycle tests cover transport and incremental state.
- Architecture checks reject duplicate authorities and host-dependent inference.
- Performance gates bound semantic work, transfer, retained memory, and latency.
- Release qualification rebuilds committed WASM on x86_64 Linux and records the
  source commit and artifact digests.

The [test matrix](capability-test-matrix.md) assigns the smallest authoritative
layer to each behavior. The [lessons](lessons.md) guide scope and evidence review.
