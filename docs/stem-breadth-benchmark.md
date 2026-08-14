# Practical STEM breadth benchmark

The practical STEM breadth benchmark is a reviewed public-development contract,
not a field-completeness score or a release holdout. It makes the next semantic
work observable across five program fields and ten capabilities before adding
new packs or inference.

The checked-in [`fixtures/development/stem-breadth-v1.json`
manifest](../fixtures/development/stem-breadth-v1.json) references exact probes
in the existing authored development fixture instead of copying source or
expectations into a second corpus.

## Matrix

The five program fields are shared foundations, linear algebra, differential
equations, probability and statistics, and numerical analysis. Each has one
cell for:

- vocabulary;
- typing;
- relation recognition;
- equivalent forms;
- conditions;
- document attachment;
- project lifecycle;
- decision quality;
- navigation;
- refusal.

A `measured` cell names reviewed public-development probes. A
`commissioned-gap` cell has no substitute evidence: it states the missing
scenario and links the implementation issue that must add independently
reviewed coverage. A raw law count, a repeated probe, or an unrelated field
cannot silently fill a gap.

The source fixture is project-original and contains no imported text. Every
referenced scenario retains its engine-blind author, independent critic, main
reviewer, exact source anchors, expectations, and review digests. The validator
rejects missing probes, duplicate cells, incomplete decision breadth, non-
independent review identities, and any attempt to use a frozen holdout as the
editable development source.

## Scoring

The benchmark reuses the authored evaluator's first-loss record rather than
constructing another semantic engine. A cell passes through a capability only
when the probe either passes completely or its first failure occurs after that
capability's authoritative layer. For example, a later decision miss does not
erase successful source attachment, while a typed-fact loss cannot count as
successful typing or law recognition.

Reports preserve separate counts for every field, capability, and cell. Probes
may intentionally support more than one cell, so field and capability totals
are diagnostic projections and must not be summed into an accuracy number.
The unique-probe result remains visible separately. Current dated measurements
belong in the [pack maturity report](pack-maturity.md), not in this policy.

Run the fast contract gate or the deliberate engine baseline with:

```sh
bun run stem:fixture
mkdir -p .artifacts && bun run stem:baseline
```

The fast gate is part of `bun run check`. The engine baseline is part of the
manual `bun run quality` workflow and writes
`.artifacts/stem-breadth-baseline.json`; generated reports are not committed.

## Fresh release boundary

The manifest commissions a new, still-unwritten sealed evaluation. Its authors
and critics must be independent, it must receive no engine execution before
seal, and historical or exposed blind fixtures are forbidden tuning inputs.
The existing semantic-release orchestrator remains the sole one-shot execution
boundary: it requires a new receipt path, runs every pre-blind gate first, and
creates an immutable terminal receipt immediately before the first engine
query.

Commissioning is not evidence. Only a future fixture that passes its schema,
review, isolation, seal, lifecycle, x86_64 WASM, and receipt checks can support
the release issue. Public development may guide implementation; the sealed
fixture may not.
