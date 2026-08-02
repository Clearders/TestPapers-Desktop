# TestPapers Desktop

Desktop application repository for the TestPapers Platform v2.

> Status: the generated Rust Cloud API contract package exists; the Tauri/Vue application scaffold remains deferred.
> Runtime owner: Desktop team.
> Release unit: signed Desktop installer and its bundled Rust Local Engine.
> Bootstrap issue: [CLE-58](https://linear.app/clearders/issue/CLE-58).

## Responsibilities

This repository will own the Tauri/Vue Desktop UI, Rust Local Engine, SQLite migrations, offline question-bank and paper-authoring workflows, local generation/export, backup/restore, and the Desktop synchronization adapter.

The canonical repository topology, runtime boundaries, and dependency direction are defined by [ADR-0001](https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md).

## Current scope

The M1 baseline contains repository governance plus the standalone `contracts/cloud-api-rust` package generated from the pinned Cloud OpenAPI contract. The package is a Rust 1.94.1 `reqwest` client and includes a small handwritten native adapter that injects Bearer authentication and returns download bytes with their response headers intact.

The repository still intentionally does not contain:

- a Tauri, Vue, Node, or SQLite application scaffold;
- Desktop application or Local Engine source;
- signing keys, update credentials, cloud tokens, or other secrets.

[CLE-23](https://linear.app/clearders/issue/CLE-23) will generate the Tauri/Vue shell in this existing repository. [CLE-24](https://linear.app/clearders/issue/CLE-24) and [CLE-25](https://linear.app/clearders/issue/CLE-25) will add the Local Engine IPC and SQLite data layer.

## Dependency rules

- Do not add source-level relative-path dependencies on `TestPapers`, `TestPaper-backend`, or `TestPapers-Mobile`.
- Consume Cloud behavior only through a pinned, versioned OpenAPI contract or generated client established by [CLE-14](https://linear.app/clearders/issue/CLE-14).
- Do not import SQLAlchemy models, Alembic migrations, Cloud repositories, Celery tasks, or Redis configuration.
- The future Vue UI must use typed, allowlisted Tauri commands/events; it must not issue SQL or unrestricted filesystem operations.
- Port reusable behavior with provenance and parity tests instead of coupling repository checkouts.

## Repository validation

Run the repository and Cloud contract checks locally:

```bash
python scripts/check_repository_baseline.py --repository TestPapers-Desktop
python scripts/check_cloud_api_rust.py
cargo fmt --manifest-path contracts/cloud-api-rust/Cargo.toml -- --check
cargo check --manifest-path contracts/cloud-api-rust/Cargo.toml --locked
cargo test --manifest-path contracts/cloud-api-rust/Cargo.toml --locked
```

Contract regeneration requires Java 17 or newer and the pinned Rust 1.94.1 toolchain:

```bash
python scripts/regenerate_cloud_api_rust.py
```

Regeneration uses only the committed `contracts/openapi.json`, `contracts/contract.lock.json`, and `contracts/openapi-generator-config.json`; it never reads another repository checkout. The lock pins the backend release identity, contract SHA-256, OpenAPI Generator 7.24.0 JAR SHA-256, and the Rust/reqwest settings.

The `Repository baseline` and `Cloud API Rust contract` GitHub checks run for pull requests and pushes to `main`. Application-specific checks will be added by their owning Linear issues.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the change workflow and [SECURITY.md](SECURITY.md) for vulnerability reporting.
