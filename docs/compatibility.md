# Compatibility and release policy

| Semath | Protocol | Pack schema | wasmtex syntax |
| --- | ---: | ---: | ---: |
| `0.18.0` candidate | 17 | 12 | 8 |

`package.json` pins the reviewed wasmtex commit. That commit, the generated
WASM declarations, protocol version, and pack schema are one tested set.

Protocol 17 gives every interpretation evidence item revision-qualified source
anchors with its own file, path, range, scope, lifecycle, and authored or
generated status. It also separates candidate, evidence, and discriminator
caps from unrelated authoring-view truncation and derives source-meaning
support from evidence authority. Protocol 16 hosts must hard-cut over with the
corresponding WASM artifact; there is no mixed-version compatibility layer.

Semantic-release outcomes are recorded by the one-shot workflow in the
permanent release ledger. A checked-in package version remains candidate
metadata until that workflow succeeds. Only then may the exact retained
package be published with its npm version, Git tag, and GitHub Release; hosts
must not pin an unpublished candidate.

Fresh release-envelope schema 2 is the immutable v0.41 contract. It replaces a
guessed full internal authoring-context golden with a sealed safety envelope.
Release-envelope schema 3 and authored-fixture schema 2 apply from v0.42: the
cursor entity decision and the selected formula disposition are reviewed and
scored independently, and every formula expectation names one exact syntax
math root. Receipt policy 3 retains the structured authoring-safety result;
policy-2 receipts remain readable as immutable evidence. None of these
evaluation contracts changes protocol 17.

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
