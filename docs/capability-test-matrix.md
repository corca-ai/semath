# Capability and test-layer matrix

The lowest layer that can state a behavior owns its permutations. E2E tests are
reserved for real editor, Worker, and deployment wiring.

| Capability | Authoritative test | Boundary evidence | E2E responsibility |
| --- | --- | --- | --- |
| Notation CST, UTF-16, cursor paths, malformed input | wasmtex contract plus `bun run notation:conformance` matrix/generative tests | adapter and clean/incremental parity | editor selection wiring |
| Semantic selection and binders | Rust cursor/parser/binder properties plus the neutral eight-family cursor plan | 102 native/WASM view/navigation queries | one semantic selection journey |
| Definitions, references, rename | pure scope and include-order tests | LSP mapping and both cursor edges | one navigation journey |
| Canonical meaning and typed laws | Rust canonical/unifier tests plus manifest-owned corpus | protocol and native/WASM equality | one meaning-first view |
| Shapes, quantities, roles, diagnostics | pure extractors and contradiction tests | Worker/LSP result mapping | reveal one source-linked conflict |
| Domain packs | Rust schema-9 compiler tests, conformance, pack-derived property planning, and evaluated or probe corpus | clean package and compiled catalog | none |
| Incremental analysis | pure six-family lifecycle planning, first-divergence comparison, shrinking, reverse-include closure, and clean-rebuild equivalence | fixed-sample and manual full-lifecycle parity plus 61/501-document budgets | one rapid-edit wiring case |
| Worker lifecycle | pure queue and generation policy tests | real engine recreation | one project-switch or crash case |
| CorTeX formula meaning | pure calm-presentation and bounded view-model tests | component integration tests | one meaning and one real-conflict journey |

New E2E permutations must identify a browser-only risk not covered below that
boundary. Pack and notation combinations belong in corpus tests.
