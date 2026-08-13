# Sync persistence and restart recovery

CLE-91 adds the durable Desktop boundary for Sync v1. It does not send network requests; the
pull/apply/ack/push worker is delivered separately by CLE-89.

## Durable records

- `sync_devices` stores the account/device binding, protocol version, authentication state, last
  acknowledged cursor, and a separately staged pulled cursor. A pulled cursor becomes acknowledged
  only through an explicit local commit.
- `pending_mutations` retains the protocol-neutral entity payload and adds device assignment, batch
  position, delivery state, dependency IDs, attempts/backoff, request hash, and the last stored
  response. Existing v1 rows migrate deterministically to `pending` with no payload rewrite.
- `sync_operation_dependencies` provides indexed dependency lookup.
- `sync_operation_results` is an exact-response journal that deliberately outlives queue cleanup.
  Reusing an operation ID with different request or response content is rejected.
- `sync_conflict_baselines` preserves base, local, and Cloud snapshots for later conflict UX.
- `sync_snapshot_rebuilds` and `sync_snapshot_entries` isolate snapshot downloads from live entity
  projections until a later atomic apply/swap step.
- `sync_runtime_state` persists the `idle -> pull -> apply -> ack -> push -> settle` phase and active
  batch without coupling storage to a transport implementation.

Queue states are `pending`, `in_flight`, `retrying`, `conflict`, `failed`, and `settled`. Unconfirmed
operations are never removed by startup recovery.

## Startup recovery

Every database open runs one immediate SQLite transaction after migrations and integrity checks:

1. `in_flight` operations become `retrying`, retain their payload and attempt count, and receive an
   immediate retry timestamp.
2. Non-idle runtime phases reset to `idle` and clear the active batch.
3. Snapshot rebuilds interrupted during `applying` or `swapping` return to `ready`; staged entries
   remain intact.

The transition is idempotent. A second startup makes no additional changes. The recovery report is
available to the Local Engine for diagnostics without exposing payloads.

## Migration and rollback

Schema v2 is additive. Desktop copies a v1 database into a staging file, applies the migration,
backfills `updated_at` from each operation's original `created_at`, runs integrity and foreign-key
checks, and only then swaps files. The untouched v1 database is retained as the rollback artifact,
so downgrading restores the original queue rather than trying to destructively reverse new tables.
