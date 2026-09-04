# Conservative mathematical document analysis

This document defines Semath's supported product scope.

Semath tracks explicit mathematical evidence in Markdown and LaTeX. It helps an
author navigate definitions, inspect declared constraints, and review changes.
CorTeX is the first host; editing, presentation, and writing suggestions remain
host responsibilities.

## Supported work

| Author task | Required evidence | Result when evidence is missing |
| --- | --- | --- |
| Find a definition or reference | An explicit declaration and an unambiguous scoped source identity | Refuse navigation; never guess from spelling or a likely field |
| Review a symbol rename | A complete editable reference set, current revisions, and no capture | Refuse the whole edit with a reason |
| Check shape, physical dimension, or unit consistency | Explicit declarations and a supported typed operation | Return partial information without a warning |
| Inspect the effect of an edit | The current snapshot and its actual dependency edges | Retract dependent results; preserve unrelated identities |

Bounded English declaration forms such as `Let $A$ be an $m$ by $n$ matrix`
are input adapters. They are not a general English comprehension promise.
Scopes, source order, negation, modality, and exact attachment must be respected.
When an adapter cannot identify the subject or assertion, it must abstain.

## Authority

A symbol definition states what an author calls a source occurrence. It does
not prove a neighboring equation. A formula can be established only by a
supported typed relation with independently grounded roles and verified
conditions. Calling an equation a law, model, identity, or result does not
satisfy those obligations.

Conflicts require incompatible grounded facts or a demonstrably invalid typed
operation. A sentence calling something incorrect or contradictory is not a
computed mathematical conflict. Questions, hypotheses, reported judgments, and
unrelated alternatives cannot determine a formula's disposition.

Structural alternatives must actually occur in the mathematical syntax, or
have independently supported typed interpretations. The engine never invents
alternatives from phrases such as “two readings remain live”.

Unrecognized input is normal. Missing declarations, unmet conditions, and
analysis limits are information about the analyzer, not defects in the
document. Only verified edit proposals are actionable, and hosts must still
review and apply them against the same source revision.

## Boundaries

Typed law matching exists to inspect explicit constraints and their evidence.
Pack vocabulary and domain routing cannot supply missing declarations or grant
edit authority. The analyzer does not propose scientific meanings from familiar
notation, infer an author's intended argument, or generate writing suggestions.

New capabilities require a concrete author task and independently reviewed
examples. The [lessons](lessons.md) explain why recognition counts and synthetic
scores cannot substitute for that evidence.

## Verification

`bun run conservative` checks small public TeX/Markdown contracts through the
real syntax adapter, native engine, and shipped WASM. It includes useful
positive behavior and refusal, not just absence of diagnostics.

`bun run check` checks code, supported behavior, parity, package integrity, and
local performance. `bun run quality` adds complete lifecycle and stable
performance checks. Release qualification uses `bun run release:check` on
x86_64 Linux and records the exact source and artifact identities.

Production usefulness requires review on real author documents;
a passing repository suite does not claim that validation has occurred.
