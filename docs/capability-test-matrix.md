# Capability and test-layer matrix

The lowest layer that can state a behavior owns its permutations. E2E tests are
reserved for real editor, Worker, and deployment wiring.

| Capability | Authoritative test | Boundary evidence | E2E responsibility |
| --- | --- | --- | --- |
| Syntax, UTF-16, selection, binders | wasmtex contract plus Rust cursor/parser/binder tests | adapter and native/WASM parity | editor selection wiring |
| Definitions, references, rename | pure scope and include-order tests | LSP mapping and both cursor edges | one navigation journey |
| Canonical meaning and typed laws | Rust canonical/unifier tests plus manifest-owned corpus | protocol and native/WASM equality | one meaning-first view |
| Shapes, quantities, roles, diagnostics | pure extractors and contradiction tests | Worker/LSP result mapping | reveal one source-linked conflict |
| Domain packs | Rust schema-5 compiler tests, conformance, and evaluated or probe corpus | clean package and compiled catalog | none |
| Incremental analysis | pure reverse-include closure and clean-rebuild equivalence | full-path 61-document budget and 501-document scale budget | one rapid-edit wiring case |
| Worker lifecycle | pure queue and generation policy tests | real engine recreation | one project-switch or crash case |
| CorTeX semantic view | pure view-model state tests | component integration tests | one cursor-to-evidence journey |

New E2E permutations must identify a browser-only risk not covered below that
boundary. Pack and notation combinations belong in corpus tests.
