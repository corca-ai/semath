# Compatibility and release policy

| Semath | Protocol | Pack schema | wasmtex syntax |
| --- | ---: | ---: | ---: |
| `0.19.3` | 18 | 13 | 8 |

`package.json` pins the reviewed wasmtex commit. The dependency, generated WASM,
protocol, and pack schema form one tested set. Hosts must adopt matching
protocol and artifact versions; mixed-version queries fail explicitly.

Before 1.0, correctness and a concise architecture take precedence over public
compatibility. A minor release may remove APIs. Wire changes increment the
protocol; pack data changes increment the pack schema. Patch releases preserve
the active contract.

## Qualification

The [supported scope](conservative-analysis.md) defines the release contract.
`bun run release:check` qualifies a clean, committed candidate on x86_64 Linux.
It rebuilds WASM, requires byte identity with the committed artifact, and runs
code, acceptance, foundation, full parity/lifecycle, stable performance,
package, and documentation checks.

Qualification writes `.artifacts/conservative-release.json` with the exact
commit, package version, dependency revision, artifact digests, and host.
A new attempt invalidates the previous report before checks begin. The command
does not publish packages, tags, releases, or messages. Publication and host
adoption must use the exact qualified source and artifact.

Release WASM is built on x86_64 Linux, never Apple Silicon. Use
`SEMATH_BUILD_HOST=<host> scripts/build-wasm-remote.sh`, then commit the tested
source and artifact together before qualification. Local performance on other
hosts is diagnostic as described in [performance gates](performance.md).
