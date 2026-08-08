# Capability and test-layer matrix

The lowest layer that can state a behavior owns its permutations. Browser E2E
is reserved for discovery and real editor/Worker wiring.

| Capability | Pure/core or unit authority | Contract/integration evidence | Representative E2E responsibility |
| --- | --- | --- | --- |
| Parsing, selection, binders, UTF-16 ranges | Rust parser/binder/source-index tests and native fixtures | native/release-WASM parity | Monaco selection wiring only |
| Definitions, hover, references, roles, shapes | Rust scope/prose/shape/consistency tests and multi-file corpus | protocol, Worker, LSP result mapping | one hover → definition → references journey |
| Diagnostics and explanation | strong-evidence Rust rules and zero-false-positive corpus | Worker/LSP diagnostics mapping | one visible problem/reveal journey |
| Formula recognition | unified pack loader, matcher registry, per-entry corpus | native/WASM parity and Worker/LSP public shapes | representative results from more than one pack |
| Completion | pure compatible-symbol enumeration and exact proposal tests | Worker/LSP completion mapping | discovery → review → stale check → apply/undo |
| Rewrite | pure refinement gates, template expansion, exact-range corpus | Worker/LSP code-action mapping | one reviewed rewrite journey |
| Rename | Rust capture/scope policy and edit-plan fixtures | Worker/LSP prepare/rename mapping | one reviewed rename journey |
| Math Inspector | pure CorTeX view model for ordering, states, evidence, and read-only policy | component localization/disclosure tests | one meaning-first cursor/action journey |
| Worker scheduling and recovery | pure queue, generation, and lifecycle transitions | port serialization and actual engine recreation | one project-switch or crash-recovery wiring case |
| Pack distribution | pure Rust/TypeScript validation and corpus gates | tarball install, exports, ABI/checksum checks | none |

Any new E2E permutation must identify a browser-only risk not already specified
by the lower layers above. Pack-entry combinations belong in corpus/conformance
tests, not browser automation.
