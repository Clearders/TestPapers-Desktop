# Environment and toolchain

## Desktop shell toolchain

| Tool | Pinned line | Ownership |
| --- | --- | --- |
| Node.js | 24.x (`.node-version`) | Vue, Vite, Vitest, ESLint, and Tauri CLI |
| npm | lockfile committed | JavaScript dependency resolution |
| Rust | 1.94.1 (`rust-toolchain.toml`) | Tauri application and tests |
| Java | 21 in contract CI only | OpenAPI Generator drift check; not needed at runtime |
| Python | repository tooling only | Baseline and environment checks; not needed at runtime |

The shell is built from this repository alone. It has no relative-path dependency on another TestPapers checkout and does not require Cloud availability, Nuxt SSR, PostgreSQL, Redis, Celery, object storage, Java, or Python to start.

The implementation was bootstrapped against the latest available `create-tauri-app` release, 4.6.2. The requested 4.7.3 version was not published in the npm registry on 2026-08-09. Runtime packages are independently and exactly pinned in `package.json` and `src-tauri/Cargo.toml`.

## Five configuration profiles

Every TestPapers repository uses `TESTPAPERS_ENV` with these meanings:

| Profile | Purpose | Safety rule |
| --- | --- | --- |
| `local` | Developer machine and local data | Safe sample defaults are allowed. |
| `development` | Shared development deployment | Use named, non-production resources. |
| `test` | Automated or isolated verification | Use disposable resources and deterministic configuration. |
| `staging` | Pre-production integration | Use HTTPS endpoints and non-production credentials supplied outside Git. |
| `production` | Live service or shipped application | Values are deployment-managed; no secrets are committed. |

The committed `.env.example` remains the forward-looking Desktop environment contract. CLE-23 does not load it: the minimal shell has no data, Local Engine, or Cloud startup. Future work may consume the reserved settings while preserving `offline` as a no-network mode. Authentication credentials must come from a future operating-system secure store, never `.env`.

The Rust Local Engine is in-process and has no URL or listening-port setting. `TESTPAPERS_CLOUD_LOCAL_API_BASE` names an optional developer Cloud endpoint only; it is ignored in `offline` mode.

Validate the reserved schema with:

```bash
python scripts/check_environment_contract.py
python -m unittest tests/test_environment_contract.py
```

## Repository matrix

| Repository | Current pinned toolchain | Gate |
| --- | --- | --- |
| TestPapers Web | Node.js 24.x | `npm run verify` |
| TestPaper Backend | CPython 3.13 | `python scripts/check.py` |
| TestPapers Desktop | Node.js 24.x + Rust 1.94.1 | frontend verify, Rust test/clippy, real-binary smoke |
| TestPapers Mobile | Dart 3.12.2; Java 21 for contract CI | Mobile-owned checks |

Repositories are independently started and verified.
