# Real TeX document evaluation

The [conservative analyzer](conservative-analysis.md) needs useful source-linked
results on author documents. This development corpus measures concrete navigation
tasks and exposes unsupported inputs. It is not a blind accuracy benchmark or an
author usability study.

## Sources and expectations

[Sources](../fixtures/real-documents/sources.json) pins five arXiv papers in machine
learning, optimization, statistics, theoretical physics, and number theory.
Selection sought different notation, author macros, and single-file and included
projects, before consulting Semath results. These are convenience samples, one
paper per field; machine learning, optimization, and statistics overlap. The first
submitted versions are used except for the Transformer paper's seventh version.
The corpus does not represent every field or establish the papers' correctness.

Each source records its authors, title, version, source URL, arXiv distribution
license, archive digest, main file, and individual source digests. The arXiv
nonexclusive distribution license is not a license for Semath to redistribute
these papers. Original archives and extracted files stay under ignored
`.artifacts/real-documents/`; the repository contains metadata and annotations.
The collector reads pinned regular files without executing TeX or extracting
arbitrary archive paths. It rejects changed content and bounded archive violations.

[Tasks](../fixtures/real-documents/tasks.json) records 14 definition destinations
and one inactive-source refusal task. The original 15 destinations were selected
from source before the first engine run; subsequent context review found that
`primes-convolution` is inside a `comment` environment (offsets 8812..13517).
Its former success label was invalid. This correction changes the historical
navigation baseline from 4/15 to 3/14 and adds a separate failed refusal task;
it does not count as improved navigation. Offsets are UTF-16,
zero-based, half-open, with original line endings preserved. They are source-first,
agent-authored annotations, not independent human labels. Inspect the declaration
and use in context before adding or correcting an annotation. Keep a substantive
reason for corrections; engine output alone is never the reason.

## Reproduce

From the repository root, with Bun, Python 3, and Rust available:

```sh
bun install
bun run corpus:collect
bun run corpus:evaluate
```

Collection requires network access on the first run; cached archives are still
verified. Evaluation uses the complete supplied TeX inventory, including source
files not included by the main document, and supplied style/class files. It passes
that inventory and the original main path through wasmtex and Semath without
flattening includes, expanding macros in a second parser, or rewriting prose.
Images and bibliographies are retained when pinned but are not semantic inputs.
External TeX distribution packages are not fetched or expanded.

The runner builds an optimized native executable and compares its public queries
with committed release WASM. Never build release WASM on Apple Silicon; follow
[release qualification](compatibility.md). Per-document subprocesses cap the
combined parse/native/WASM work at 120 seconds; the native call has a 60-second
limit. Rejected or timed-out documents remain in the task denominator. Unexpected
process failures, changed source, stale annotations, or parity mismatches fail the
command. Earlier completed observations are written after each document.

The generated `.artifacts/real-documents/report.json` records source and artifact
identity, dirty checkout status, platform, exact query results, and diagnostics.
Elapsed time includes parsing and both runtimes, so it is a troubleshooting
observation, not editor latency or a replacement for [performance gates](performance.md).

## Read the results

A navigation task succeeds only when its single destination exactly matches the
annotated file and range. Report correct, abstained, wrong-target, rejected, and
timed-out outcomes separately, both per field and across all 14 navigation tasks.
The inactive-source task passes only when both navigation and rename refuse,
with no destination or edit proposal. Report it separately as `correct-refusal`
or `unexpected-authority`; never add a refusal to the navigation numerator. A successful
process or conservative refusal alone is not a useful navigation result.

Rename queries propose `w` and retain the response for inspection. Their complete
reference sets have not been manually annotated; neither authorization nor refusal
is scored as rename correctness. Do not infer safe edits from navigation success.
Likewise, review diagnostics against their evidence in the original document;
publication is not an oracle that every warning is false. This corpus currently
has no independently labeled shape/unit conflict set or author-time measurements.
Those capabilities remain covered by the smaller [acceptance tests](capability-test-matrix.md),
without a real-document accuracy claim.

Use a confirmed failure to add a small regression at the responsible layer, fix
its cause, and rerun the original document without changing its annotation.
The first such fix prevents concessive conditions such as “even with” from
becoming global comparison facts. The first findings and remaining work are tracked in
[issue 405](https://github.com/corca-ai/semath/issues/405). Keep generated reports local; run `bun run check`, `bun run quality`, and
`awiki lint -r` when changing the corpus or semantic behavior.
