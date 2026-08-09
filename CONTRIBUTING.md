# Contributing to TestPapers Desktop

## Before starting

1. Work from a Linear issue with acceptance criteria and dependencies.
2. Confirm the issue belongs to the Desktop repository and does not move Web or Cloud source.
3. Use a branch containing the issue identifier, for example `cle-23-tauri-shell`.

## Change workflow

1. Branch from the current protected `main`.
2. Keep one primary pull request per Linear issue; link companion PRs when a change spans repositories.
3. Run `npm run verify`, the Rust fmt/clippy/test gates, and `python scripts/check_repository_baseline.py --repository TestPapers-Desktop` before pushing.
4. For native changes, run `npm run build:desktop` and `node scripts/smoke-desktop.mjs`; include manual platform evidence when behavior is platform-specific.
5. Complete every section of the pull request template and request a code-owner review.

## Architecture rules

- Preserve the boundaries in [ADR-0001](https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md).
- Do not introduce relative-path source dependencies on another TestPapers application repository.
- Keep credentials, signing material, update keys, and production configuration out of Git.
- Prefer typed, narrow interfaces and backward-compatible Cloud contract changes.
- Keep Tauri calls inside the single typed frontend adapter and update the capability allowlist for every IPC change.
- Record provenance and parity tests when porting behavior from Web or Cloud.

## Commit and pull request quality

- Write focused commits with imperative subjects; include the Linear identifier in the PR title.
- Document user-visible behavior, compatibility, migration, security, and rollback impact.
- Do not merge with failing required checks, unresolved review threads, or undocumented high-risk follow-up work.
