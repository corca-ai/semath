# Capability and test-layer matrix

The lowest layer that can state a behavior owns its permutations. Browser E2E
is reserved for discovery and real editor/Worker wiring.

| Capability | Pure/core or unit authority | Contract/integration evidence | Representative E2E responsibility |
| --- | --- | --- | --- |
| Parsing, selection, binders, UTF-16 ranges | Pure cursor policy plus Rust parser/binder/source-index boundary and generated invariant tests | native/release-WASM and LSP UTF-16 parity | Monaco selection wiring only |
| Definitions, hover, references, roles, shapes | Pure include-order/visibility index plus Rust scope/prose/shape/consistency tests, multi-file corpus, and independently authored English prose recognition/refusal/coverage cases | protocol, Worker, LSP result mapping | one hover → definition → references journey |
| Diagnostics and explanation | strong-evidence Rust rules and zero-false-positive corpus | Worker/LSP diagnostics mapping | one visible problem/reveal journey |
| Formula recognition | one structure-anchored matcher registry; compact generated permutations plus independently authored domain recognition/refusal/coverage cases and exact scorecards | native/WASM exact parity and Worker/LSP public shapes | representative results from more than one pack |
| Completion | pure compatible-symbol enumeration and exact proposal tests | Worker/LSP completion mapping | discovery → review → stale check → apply/undo |
| Rewrite | pure refinement gates, template expansion, exact-range corpus | Worker/LSP code-action mapping | one reviewed rewrite journey |
| Rename | Rust capture/scope policy and edit-plan fixtures | Worker/LSP prepare/rename mapping | one reviewed rename journey |
| Math Inspector | pure CorTeX view model for ordering, states, evidence, and read-only policy | component localization/disclosure tests | one meaning-first cursor/action journey |
| Worker scheduling and recovery | pure queue, epoch/inventory/generation fences, and lifecycle transitions | port serialization, actual engine recreation, and realistic seven-file project | one project-switch or crash-recovery wiring case |
| Pack distribution | pure Rust/TypeScript validation and corpus gates | tarball install, exports, ABI/checksum checks | none |

Any new E2E permutation must identify a browser-only risk not already specified
by the lower layers above. Pack-entry combinations belong in corpus/conformance
tests, not browser automation.

The synthetic corpus is intentionally checked in rather than regenerated during
tests. Its LLM authoring lanes are isolated by field; the executable harness
only validates annotations and measures the current supported fraction. This
keeps coverage discovery independent from implementation-owned fixture
generation.
