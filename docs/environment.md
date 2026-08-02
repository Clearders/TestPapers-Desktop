# Environment and toolchain

## Five configuration profiles

Every TestPapers repository uses `TESTPAPERS_ENV` with exactly these meanings:

| Profile | Purpose | Safety rule |
| --- | --- | --- |
| `local` | A developer machine and local data | Safe sample defaults are allowed. |
| `development` | A shared development deployment | Use named, non-production resources. |
| `test` | Automated or isolated verification | Use disposable resources and deterministic configuration. |
| `staging` | Pre-production integration | Use HTTPS endpoints and non-production credentials supplied outside Git. |
| `production` | Live service or shipped application | Values are deployment-managed; no secrets are committed. |

Configuration belongs in an ignored `.env`; `.env.example` is the reviewable schema. Missing or invalid required configuration is an error, never a fallback.

## Shared four-repository toolchain matrix

| Repository | Current pinned toolchain | Lock / ownership boundary |
| --- | --- | --- |
| TestPapers Web | Node.js 24.x in CI | `package-lock.json`; `npm run verify` is the repository gate. |
| TestPaper Backend | CPython 3.13 in CI | `uv.lock`; `python scripts/check.py` is the repository gate. |
| TestPapers Desktop | Rust 1.94.1 in contract CI; Java 21 in CI | `Cargo.lock` pins the generated client; Python is repository-validation tooling only; the Tauri runtime is deferred to CLE-23. |
| TestPapers Mobile | Dart 3.12.2 in contract CI; Java 21 in CI | `pubspec.lock` pins the generated client; Python is repository-validation tooling only; the Flutter runtime is deferred to CLE-35. |

Repositories are independently started and verified. They do not require relative-path checkouts of one another.

## Desktop settings

Copy `.env.example` to `.env`, then choose `offline`, `local`, `staging`, or `production` Cloud API mode. Desktop data, SQLite, and export locations are local filesystem paths. `offline` uses no Cloud endpoint. `local` accepts a credential-free HTTP(S) origin; `staging` and `production` use distinct HTTPS origins. The matching deployment profile requires its matching API mode; `test` permits only `offline` or `local`. Authentication credentials are injected by the future application secure store, never by `.env`.

Desktop does not configure PostgreSQL, Redis, Celery, Python runtime, or object storage. Validate the committed schema with:

```bash
python scripts/check_environment_contract.py
python -m unittest tests/test_environment_contract.py
```
