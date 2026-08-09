# Documentation Guide

This document defines how to write and maintain project docs.

## Goal

Keep docs easy to scan and easy to trust for both humans and agents.

## Principles

- Treat docs like code.
- Keep intent explicit and concise.
- Prefer small focused docs over long mixed docs.
- Link related docs, but keep navigation short.
- Do not version control auto-generated docs.
- Distinguish code defaults, approved rollout policy, and dated live production
  state. Never present one as another.
- Keep durable architecture separate from plans. Record current plans and
  acceptance criteria in GitHub issues.

## Structure

- Keep the root `README.md` minimal; point readers to the
  [documentation index](index.md) for onboarding and navigation.
- Keep `AGENTS.md` as working rules and a compact agent-oriented project map.
- Use the [documentation index](index.md) as the human onboarding entry point
  and complete task-oriented catalog. Every first-level `docs/*.md` file must be
  discoverable from it.
- Keep the structure under `docs/` as flat as practical. Add a subdirectory only
  for a cohesive document family that would make the first level harder to scan.
- Give each document one primary purpose. Split architecture, plans, operations,
  API contracts, and historical records when they begin to compete for space.

## Maintenance

- Update [docs/index.md](index.md) whenever a first-level document is added,
  renamed, or removed.
- Prefer relative links between repository documents.
- State whether descriptions are current behavior, durable design, historical
  context, or future work.
- Keep commands executable and update them with the code they describe.
- Run `awiki lint -r` from the repository root after changing documentation;
  recursive mode includes `docs/` and nested corpus guides.
- Run relevant code tests as well when documentation contains executable
  examples or changes a public contract.
