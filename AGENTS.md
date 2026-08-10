# Project agent guide

Semath is a host-independent Rust/WASM semantic engine, not a web application.
CorTeX is the first host.

## Read first

- Use [`docs/index.md`](docs/index.md) as the documentation entry point.
- Treat [`docs/architecture.md`](docs/architecture.md) as durable design and
  GitHub issues as current plans.
- Follow [`docs/metadoc.md`](docs/metadoc.md) for documentation changes.

## Project map

- `crates/`: Rust semantic core plus native and WASM boundaries.
- `packages/`: protocol, adapters, Worker, LSP, and pack tooling.
- `packs/` and `schemas/`: versioned domain knowledge and contracts.
- `fixtures/`: corpus, parity, and performance cases.
- `scripts/`: verification and release tooling.
- `docs/`: maintained project documentation; keep it flat when practical.

## Working rules

- Prefer pure, bounded, evidence-preserving core transformations.
- Preserve native/WASM behavior parity. Prefer the correct architecture and
  concise code over compatibility; breaking API and host UI changes are allowed.
- Add focused tests at the lowest authoritative layer; minimize E2E coverage.
- Run the fast `bun run check` for code changes and `awiki lint -r` for
  documentation. Run the deliberately manual `bun run quality` before a
  semantic release or when changing packs, corpora, inference, or thresholds.
- Build release WASM on an x86_64 Linux host, never on Apple Silicon.
- `CLAUDE.md` is a symlink to `AGENTS.md`; edit `AGENTS.md` and preserve the link.
