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

## Conservative qualification

The [supported scope](conservative-analysis.md) defines the release contract.
`bun run release:check` (also available as `release:semantic`) qualifies a clean,
committed candidate on x86_64 Linux. It rebuilds WASM, requires byte identity
with the committed artifact, and runs code, conservative acceptance,
foundation, full lifecycle/parity, stable performance, package, and docs checks.

Qualification is repeatable. It writes `.artifacts/conservative-release.json`
with the exact commit, package version, dependency revision, artifact digests,
and host identity. A new attempt invalidates a previous successful report
before checks begin. Qualification does not publish a package, tag, release,
or message. Publication and host adoption must use the exact qualified source
and artifact; candidate metadata alone is not release evidence.

The broad-STEM one-shot workflow has been moved outside GitHub's active
workflow directory to `.github/retired-workflows/`. Historical fixtures,
receipts, and reservation tools are retained for audit, not current release
qualification. No new blind fixture or GitHub ledger reservation is required.
Historical failures remain failures under their original policies; the
conservative scope does not relabel them as successful releases.

Formula establishment now requires independently grounded typed relations and
verified conditions. Prose descriptions and verdicts no longer establish or
conflict a formula. Hosts must not interpret `established` symbol identity as
verification of an enclosing equation. The wire shape remains protocol 17.

Before 1.0, correctness and a concise architecture take precedence over public
compatibility. A minor release may remove APIs and change host UI without a
compatibility layer. It must increment the protocol or pack schema when the
corresponding wire or data contract changes. Patch releases preserve the
active contract.

Release WASM is built on a separate x86_64 Linux host, not Apple Silicon.
Use `SEMATH_BUILD_HOST=<host> scripts/build-wasm-remote.sh` to refresh local
artifacts, then commit the tested source and artifact together before release
qualification. A native-only test run does not establish native/WASM parity.
