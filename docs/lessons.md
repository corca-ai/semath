# Lessons for conservative analysis

Semath previously pursued broad scientific recognition and writing assistance.
Generated examples and an elaborate release process obscured the gap between
recognizing familiar forms and helping with real documents. The product now
follows the [supported scope](conservative-analysis.md).

- Start with an author task and observable behavior. A larger law catalog or
  higher aggregate score is not evidence that a document tool is useful.
- Keep facts separate from guesses. Familiar notation, a field name, an author's
  verdict, and a phrase such as “this is a law” cannot prove an equation.
- Treat missing evidence as normal. Refuse unsupported navigation or edits;
  report an error only when grounded facts demonstrate a conflict.
- Specify expected results before consulting engine output. Generated cases
  catch regressions but do not independently establish accuracy. Never change
  an expectation merely because the current implementation disagrees.
- Test successful work as well as refusal. Silence alone is not correctness.
  Keep focused source-order, scope, contradiction, and retraction examples.
- Check the actual input path. Native helpers alone cannot validate the syntax
  adapter or shipped WASM. Compare clean and incremental results at boundaries.
- Keep qualification repeatable and proportional to the product. A release
  needs identifiable source, reproducible artifacts, and relevant checks.
  It does not need a growing reservation ledger or a new evaluation bureaucracy.
- Compare performance on the same platform with unchanged limits. Record host
  runtime overhead separately from live engine memory.
- Remove abandoned features and their tooling together. Do not retain a second
  product under a research label. A new capability needs an explicit scope
  decision and independently reviewed examples from actual use.

These rules guide implementation and review. The [test matrix](capability-test-matrix.md)
identifies the checks that enforce the corresponding contracts.
