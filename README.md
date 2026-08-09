# TestPapers Desktop

Tauri 2 + Vue 3 desktop application for the TestPapers Platform v2.

> Status: CLE-23 provides the minimal desktop shell; question-bank, paper-authoring, SQLite, Local Engine, synchronization, signing, and updating remain deferred.
> Runtime owner: Desktop team.
> Release unit: signed Desktop installer and its bundled Rust Local Engine.
> Repository bootstrap: [CLE-58](https://linear.app/clearders/issue/CLE-58); desktop shell: [CLE-23](https://linear.app/clearders/issue/CLE-23).

## What is implemented

- A single local-only window, initially hidden until Vue applies the effective theme and calls `frontend_ready`.
- Native application, file, theme, and tray menus, with tray-unavailable fallback behavior.
- System/light/dark theme preferences and `ask`/`quit`/`tray` close behavior persisted in the application data directory as `settings.v1.json`.
- CSV/JSON import and DOCX/TeX export dialog previews that return only cancellation state and basenames; they do not read or write files.
- A versioned, camelCase IPC boundary exposed only through `src/infrastructure/tauri/shellBridge.ts`.
- The standalone generated Cloud contract package under `contracts/cloud-api-rust`; the desktop shell does not link or start it.

The canonical topology and dependency direction are defined by [ADR-0001](https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md). See [docs/architecture.md](docs/architecture.md) for the shell boundary and [docs/ipc.md](docs/ipc.md) for the public command/event allowlist.

## Deferred scope

CLE-23 does not create a database, start a local service, call Cloud, or implement import/export contents. Those surfaces belong to CLE-24/25/26/27/52. No fs, SQL, shell, HTTP, remote WebView, global Tauri object, updater, or signing permission is enabled.

The generated Cloud package validates the hardened native API contract only. It is not a shipping authentication runtime, and no credential or token persistence is implemented here.

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
python -m unittest tests/test_environment_contract.py
```

`TESTPAPERS_DESKTOP_SMOKE=1` makes a built application exit immediately after the Vue-ready handshake. `node scripts/smoke-desktop.mjs` runs the real binary and requires both `ready` and idempotent `cleanup` log markers.

## Dependency rules

- Do not add source-level relative-path dependencies on `TestPapers`, `TestPaper-backend`, or `TestPapers-Mobile`.
- Consume Cloud behavior only through a pinned contract boundary; do not start Cloud during desktop bootstrap.
- Do not import Cloud persistence, migrations, workers, or infrastructure configuration.
- Vue components and composables must use the typed desktop adapter, never Tauri `invoke` or event APIs directly.
- Keep IPC allowlisted and DTOs versioned; never return an absolute path from a dialog preview.
- Record provenance and parity tests when porting Web behavior or assets.

See [docs/environment.md](docs/environment.md), [docs/provenance.md](docs/provenance.md), [docs/manual-smoke.md](docs/manual-smoke.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [SECURITY.md](SECURITY.md).
