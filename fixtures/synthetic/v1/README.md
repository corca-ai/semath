# Synthetic corpus v1

This corpus is authored directly by independent LLM contexts. It is not
produced by a template expander or by mutating the existing calibration
fixtures. Each authoring lane received the shared schema and only the pack or
prose contract for its assigned field; it was instructed not to inspect sibling
corpus files. The algebra and number-theory lane was authored separately from
the five existing-pack lanes.

Every case is a small standalone English/TeX document with one annotated query
site and an exact expected result. Cases have one of three purposes:

- `recognition`: a supported surface that must keep its exact pattern or prose
  definition result;
- `refusal`: a close or malformed surface that the field-specific pattern must
  refuse; an explicitly listed, more general cross-pack recognition may remain;
- `coverage`: a useful, currently unsupported concept that records the next
  semantic frontier without weakening the false-positive gate.

The checked-in JSON is the source of truth. `scripts/check-synthetic-corpus.ts`
validates the schema, cursor annotation, pattern ownership, cross-file
duplication, and exact native-engine result. It prints current semantic coverage
without generating additional examples. Real-project corpora can be added as a
separate layer later.

The English declaration lane contains 180 cases rather than template-expanded
renamings. Its diversity dimensions include:

- singular, two-symbol, and three-symbol declarations;
- shared descriptions and exact two-way or three-way mappings using
  `respectively` or `in that order`;
- `let`, direct relational, `write ... for`, imperative, contextual,
  appositional, parenthetical, quantified, notation-table, and mathematical
  assignment/declaration surfaces;
- Latin, Greek, styled, indexed, scalar, set, function, matrix, quantity, and
  engineering notation in both Markdown and LaTeX prose;
- mismatched arity, missing order evidence, modal/conditional/negated mentions,
  malformed declarations, forward references, and ambiguous glosses.

New cases must add a natural construction, structural variation, notation class,
domain context, or refusal boundary. Renaming symbols or substituting synonyms in
the same sentence frame does not qualify as independent diversity. The release
gate keeps at least 90 recognition, 45 refusal, and 30 explicit coverage cases.
