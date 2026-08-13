# TestPapers Desktop

Tauri 2 + Vue 3 desktop application for the TestPapers Platform v2.

> Status: CLE-24 through CLE-28 provide the embedded Local Engine, SQLite data layer, offline question bank, paper generation/export, and workspace backup foundation. CLE-91 and CLE-89 add durable Sync v1 delivery; CLE-97 adds portable status models and user controls. Signing and updating remain deferred.
> Runtime owner: Desktop team.
> Release unit: signed Desktop installer and its bundled Rust Local Engine.
> Repository bootstrap: [CLE-58](https://linear.app/clearders/issue/CLE-58); desktop shell: [CLE-23](https://linear.app/clearders/issue/CLE-23); local-first workspace: [CLE-24](https://linear.app/clearders/issue/CLE-24), [CLE-25](https://linear.app/clearders/issue/CLE-25), [CLE-26](https://linear.app/clearders/issue/CLE-26), [CLE-27](https://linear.app/clearders/issue/CLE-27), and [CLE-28](https://linear.app/clearders/issue/CLE-28).

## What is implemented

- A single local-only window, initially hidden until Vue applies the effective theme and calls `frontend_ready`.
- An embedded, supervised Rust Local Engine with single-instance behavior, a stable UUIDv7 workspace identity, an OS workspace lock, typed recovery states, and no local HTTP listener.
- Bundled SQLite with WAL, foreign keys, staged versioned migrations, integrity checks, entity history, restart-safe Sync v1 queue/cursor state, content-addressed attachments, and FTS5 question search.
- A generated and lock-pinned Cloud API 1.2.0 client plus an authenticated `pull -> apply -> ack -> push -> settle` worker with exact batch replay, bounded backoff, explicit conflicts, and atomic snapshot recovery.
- Global and per-entity Sync status, restart-safe pause/resume, manual sync/safe retry controls, and a JSON Schema/fixture boundary reusable by Flutter without Rust.
- Offline question creation, editing, soft deletion/restoration, revision history, cursor-based filtering, and validated CSV/JSON batch import.
- Deterministic local paper generation plus DOCX and TeX export, with a checksum-pinned, network-disabled offline PDF sidecar contract; all run as cancellable background jobs.
- Consistent workspace snapshots, validated backup archives, live automatic scheduling and verified retention, restore preflight, and maintenance-mode coordination.
- Native application, file, theme, and tray menus, with tray-unavailable fallback behavior.
- System/light/dark theme preferences and `ask`/`quit`/`tray` close behavior persisted in the application data directory as `settings.v1.json`.
- Native file selectors retain absolute paths in Rust and expose only one-time selection IDs and display-name basenames.
- A versioned, camelCase IPC boundary exposed only through the dedicated shell and Local Engine bridges.
- The standalone generated Cloud contract package under `contracts/cloud-api-rust`, linked only by the background sync boundary and never started during Desktop bootstrap.

The canonical topology and dependency direction are defined by [ADR-0001](https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md). See [docs/architecture.md](docs/architecture.md) for the runtime boundary, [docs/local-first-workspace.md](docs/local-first-workspace.md) for local data behavior, [docs/sync-persistence.md](docs/sync-persistence.md) for the durable Sync v1 state machine, and [docs/ipc.md](docs/ipc.md) for the public command/event allowlist.

## Remaining scope

The Desktop runtime does not call Cloud during bootstrap and has no generic fs, SQL, shell, HTTP, remote WebView, or global Tauri permission. Native account-session handoff, rich conflict resolution, collaborative workflows, production signing, the reviewed Tectonic binary/minimal-bundle release payload, and updater delivery remain separate release work.

The worker accepts a short-lived Bearer token from the native authentication boundary. It does not persist credentials or tokens; wiring account controls and token refresh into the shell is CLE-97 scope.

## Development

Prerequisites are Node.js 24.x, npm, Rust 1.94.1, and the platform prerequisites for Tauri 2. Java and Python are not desktop runtime dependencies; Java 21 is used only by Cloud contract drift CI, and Python is used by repository validators.

```bash
npm ci
npm run dev
```

Run the local gates:

```bash
npm run verify
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run build:desktop
python scripts/check_repository_baseline.py --repository TestPapers-Desktop
python scripts/check_environment_contract.py
python scripts/check_local_data_contract.py
python scripts/check_sync_client_state.py
python -m unittest tests/test_environment_contract.py
python -m unittest tests/test_local_data_contract.py
```

`TESTPAPERS_DESKTOP_SMOKE=1` makes a built application exit immediately after the Vue-ready handshake. `node scripts/smoke-desktop.mjs` runs the real binary and requires both `ready` and idempotent `cleanup` log markers.

## Dependency rules

- Do not add source-level relative-path dependencies on `TestPapers`, `TestPaper-backend`, or `TestPapers-Mobile`.
- Consume Cloud behavior only through a pinned contract boundary; do not start Cloud during desktop bootstrap.
- Do not import Cloud persistence, migrations, workers, or infrastructure configuration.
- Vue components and composables must use the typed shell or Local Engine adapter, never Tauri `invoke` or event APIs directly.
- Keep IPC allowlisted and DTOs versioned; never return an absolute path through IPC.
- Record provenance and parity tests when porting Web behavior or assets.

See [docs/environment.md](docs/environment.md), [docs/provenance.md](docs/provenance.md), [docs/manual-smoke.md](docs/manual-smoke.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [SECURITY.md](SECURITY.md).
