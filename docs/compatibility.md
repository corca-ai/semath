# Compatibility and release policy

| Semath | Protocol | Pack schema | wasmtex syntax |
| --- | ---: | ---: | ---: |
| current `main` code | 12 | 10 | 8 |

`package.json` pins the reviewed wasmtex commit. That commit, the generated
WASM declarations, protocol version, and pack schema are one tested set.

Before 1.0, correctness and a concise architecture take precedence over public
compatibility. A minor release may remove APIs and change host UI without a
compatibility layer. It must increment the protocol or pack schema when the
corresponding wire or data contract changes. Patch releases preserve the
active contract.

Release WASM is built on a separate x86_64 Linux host, not Apple Silicon. The
manual `bun run release:semantic` gate checks formatting, lint, unit tests,
editable development evidence and final historical regression evidence,
native/WASM parity, incremental
performance, package installation, and docs before it spends the explicitly
selected fresh blind fixture. The fresh fixture is never executed by ordinary
CI. Its immutable receipt records the exact Semath and wasmtex revisions and
native/WASM artifact digests.
