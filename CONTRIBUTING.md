# Contributing to TestPapers Desktop

## Before starting

1. Work from a Linear issue with acceptance criteria and dependencies.
2. Confirm the issue belongs to the Desktop repository and does not move Web or Cloud source.
3. Use a branch containing the issue identifier, for example `cle-23-tauri-shell`.

## Change workflow

1. Branch from the current protected `main`.
2. Keep one primary pull request per Linear issue; link companion PRs when a change spans repositories.
3. Run `python scripts/check_repository_baseline.py --repository TestPapers-Desktop` before pushing.
4. Add application-specific lint, test, build, migration, security, and packaging evidence once those surfaces exist.
5. Complete every section of the pull request template and request a code-owner review.

## Architecture rules

- Preserve the boundaries in [ADR-0001](https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md).
- Do not introduce relative-path source dependencies on another TestPapers application repository.
- Keep credentials, signing material, update keys, and production configuration out of Git.
- Prefer typed, narrow interfaces and backward-compatible Cloud contract changes.
- Record provenance and parity tests when porting behavior from Web or Cloud.

## Commit and pull request quality

- Write focused commits with imperative subjects; include the Linear identifier in the PR title.
- Document user-visible behavior, compatibility, migration, security, and rollback impact.
- Do not merge with failing required checks, unresolved review threads, or undocumented high-risk follow-up work.
