# Compatibility and release policy

| Semath | Protocol | Pack schema | wasmtex syntax | Host requirement |
| --- | ---: | ---: | --- | --- |
| 0.12.x | 1 | 2 | pinned in `package.json` | ES modules, WebAssembly, Worker where the host API is used |

The package pins the reviewed wasmtex revision used to derive syntax ranges. Consumers should treat that revision, the generated WASM declarations, and `SEMATH_PROTOCOL_VERSION` as one tested compatibility set.

Before 1.0, a Semath minor release may make a breaking public change. Patch releases preserve protocol envelopes, exported TypeScript names, pack schema behavior, and generated WASM ABI. A breaking change increments the Semath minor version and, when wire or pack data changes incompatibly, also increments the corresponding protocol or pack schema version. Fields may be added compatibly only when older consumers can safely ignore them.

Deprecations remain documented for at least one minor release where a safe compatibility path is practical. Silent reinterpretation of an existing field, maturity tier, or edit authority is not compatible.

Release WASM is built on a remote non-Apple-Silicon x86_64 Linux host. CI verifies the checked-in checksum, declarations, native/WASM parity, clean-package installation, public examples, and pack conformance before release.
