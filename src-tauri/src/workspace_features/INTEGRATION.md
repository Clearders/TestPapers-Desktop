# Workspace feature integration

`workspace_features` contains CLE-27/CLE-28 domain/application code only. It has no Tauri, dialog,
SQLite, or frontend dependency. Add `mod workspace_features;` at the crate root before wiring it.

## Job coordinator

- Construct `jobs::JobCoordinator::new(Some(event_sink))`; the `JobEventSink` adapter should emit
  `JobSnapshot` as `testpapers://jobs/updated`.
- `JobId` is UUIDv7. `JobKind` is the wire enum `import | generation | export | backup |
  restore | dataDirectoryMigration`. States are `queued | running | cancelling | completed |
  failed | cancelled`.
- Submit the four FIFO queues through `submit(JobKind, work)`. Work receives `JobContext`, reports
  progress, checks `cancellation().checkpoint()`, and calls `commit_started()` immediately before an
  irreversible transaction/atomic replacement.
- Restore and data-directory migration must use `submit_maintenance`. It cancels cooperative work,
  drains all four queues, blocks new submissions, and holds an exclusive `MaintenanceLease` until
  the maintenance job finishes.

## Papers, generation, and export

- Implement `paper::PaperSnapshotStore` with CLE-25's optimistic SQLite transaction. Persist the
  immutable `QuestionSnapshot` contained by every stable `PaperItemSnapshot`; never rehydrate an
  export from the current question row.
- Call `generation::generate(request, candidates, observer)` in a generation job. Input order is
  normalized, the seed is explicit, type counts are exact, and insufficient candidates return
  structured diagnostics without a partial paper. Convert `GeneratedQuestion` values to stable
  paper-item UUIDv7 values and accept the whole paper in one SQLite transaction.
- `export::build_tex` returns TeX plus image companions. `export::build_docx` returns a complete
  stored-ZIP OOXML document with embedded, SHA-256-verified images. Write either artifact to a
  sibling temporary file and rename only after validation.
- PDF export writes the generated TeX and companions into an isolated temporary directory and calls
  `export::BundledTectonic`. Supply release-pinned SHA-256 values for both the executable and offline
  bundle. A missing or mismatched sidecar returns `TectonicError::Unavailable`; there is no network
  or system-Tectonic fallback.

## Backup, restore, and data directory

- Implement `backup::ConsistentDatabaseSnapshot` with `rusqlite::backup::Backup`; its inventory must
  be read from the completed snapshot so only referenced blobs are archived. Include the current
  `workspace.v1.json` as the required `workspace_metadata` role so a fresh installation can restore
  the workspace identity as well as its database.
- `create_consistent_backup` produces a hash/size manifest and bounded stored-ZIP archive.
  `write_new_backup_atomically` publishes a new target without overwriting an existing file.
- Implement `backup::AgeBackend` with the audited `age` crate and
  `KeychainIdentityProvider` with the platform credential store. `SecretBytes` redacts debug output
  and zeroes its allocation on drop. Do not implement a weaker encryption fallback.
- Decrypt before `preflight_restore`; preflight verifies the archive, stages files, rejects a newer
  schema, migrates older snapshots through `DatabasePreflight`, and runs database integrity checks.
  In a maintenance job, close SQLite and call `install_preflighted_restore`. Success keeps the old
  workspace at the supplied rollback path; health-check failure reinstalls it and retains the failed
  candidate separately.
- Resolve native directory selections inside Rust, probe space/permissions/volume, create a
  `DataDirectoryPlan`, and run `migrate_data_directory` under maintenance. Implement
  `WorkspacePointer::activate` as an atomic bootstrap-settings update. The source remains untouched
  as rollback data.

## Dependencies and packaged resources

The module compiles against dependencies already selected by the central work: `serde`,
`serde_json`, and `uuid` with `serde,v7`; unit tests use `tempfile`. Production encryption needs an
audited `age` adapter plus a platform keychain adapter (for example `keyring` with `secrecy` and
`zeroize`). PDF packaging must add the platform Tectonic binary, an offline resource bundle, CJK
fonts/templates, release checksums, and license/source records. No Python, HTTP server, Cloud,
PostgreSQL, Redis, or Celery runtime is required.
