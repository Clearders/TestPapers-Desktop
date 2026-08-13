# Platform-neutral Sync client state v1

CLE-97 defines the UI-facing Sync state independently from Rust, Tauri, SQLite, and HTTP. The
canonical artifacts are:

- `contracts/sync-client-state.schema.json` — JSON Schema for snapshots and status-change events.
- `contracts/sync-client-state.fixtures.json` — one fixed vector for every public state plus an
  event vector.
- `contracts/sync-client-state.lock.json` — byte hashes and a combined semantic fingerprint.

TypeScript validates this wire shape at the Tauri boundary. Flutter can generate or hand-write the
same enums and records from the JSON Schema and must run the fixture file as its parity suite; it
does not need to link Rust or understand Desktop SQLite.

## Status precedence

The client computes one global state in this order: authentication required, conflict, failed,
syncing, transient offline/retrying, persisted retrying, pending, then synced. A separate `paused`
flag preserves the underlying data state and changes the recommended action to `resume`.

Outstanding entities carry the same public state vocabulary. Desktop currently surfaces question
states in the workspace and lists every affected entity in the Sync status dialog. Rich conflict
resolution remains a separate M4 issue.

## Controls and safety

Pause is durable in SQLite and is checked before a worker performs network I/O. Pause, offline,
authentication expiry, and device revocation never gate Local Engine editing. Manual retry only
makes the existing retry batch due; it does not rewrite payloads, operation IDs, batch IDs, or
stored results.

The native authentication boundary configures a Cloud session in memory with a short-lived Bearer
token. Snapshots and events expose only stable error codes and counts—never credentials, payloads,
filesystem paths, or Cloud response bodies.
