# Compatibility and release policy

| Semath | Protocol | Pack schema | wasmtex syntax |
| --- | ---: | ---: | ---: |
| 0.17.x | 2 | 4 | 3 |

`package.json` pins the reviewed wasmtex commit. That commit, the generated
WASM declarations, protocol version, and pack schema are one tested set.

Before 1.0, correctness and a concise architecture take precedence over public
compatibility. A minor release may remove APIs and change host UI without a
compatibility layer. It must increment the protocol or pack schema when the
corresponding wire or data contract changes. Patch releases preserve the
active contract.

Release WASM is built on a separate x86_64 Linux host, not Apple Silicon. The
release gate checks formatting, lint, unit tests, the held-out corpus,
native/WASM parity, incremental performance, package installation, and docs.
