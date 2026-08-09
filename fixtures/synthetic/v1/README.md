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
- `refusal`: a close or malformed surface that must remain unrecognized;
- `coverage`: a useful, currently unsupported concept that records the next
  semantic frontier without weakening the false-positive gate.

The checked-in JSON is the source of truth. `scripts/check-synthetic-corpus.ts`
validates the schema, cursor annotation, pattern ownership, cross-file
duplication, and exact native-engine result. It prints current semantic coverage
without generating additional examples. Real-project corpora can be added as a
separate layer later.
