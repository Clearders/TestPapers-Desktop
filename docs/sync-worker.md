# Desktop Sync v1 worker

CLE-89 implements the reusable Desktop transport and background worker for Cloud API `1.2.0`,
Sync protocol `v1`. Construction is explicit: Desktop bootstrap never starts network traffic, and
CLE-97 owns the shell controls and the platform-neutral status/event boundary. The native
authentication component provides the account, device, base URL, and short-lived Bearer token.

## Durable cycle

Each run follows `idle -> pull -> apply -> ack -> push -> settle -> idle` and persists every phase.
Network requests occur without holding the SQLite mutex, so local editing remains available on slow
or unavailable networks.

1. Pull starts from the last acknowledged opaque cursor and follows bounded pages.
2. Each page and its pulled cursor are committed in one local transaction. A remote change never
   overwrites an entity that has an unmerged local candidate; instead, Desktop stores base, local,
   and Cloud snapshots as an explicit conflict.
3. Desktop acknowledges the page remotely only after local commit, then promotes the local pulled
   cursor to acknowledged. A dropped acknowledgement safely replays the page as a hash/version
   no-op.
4. Push selects a deterministic batch and records its ID, ordinal, request hash, and attempt before
   transport. A lost response or restart retries the exact same batch. Accepted results are stored
   before operations settle, so replay cannot create a second semantic version.
5. Retryable failures use persisted exponential backoff. Authentication failure or device
   revocation stops the cycle and records a credential-required state without discarding edits.

## Snapshot recovery

An expired cursor triggers a paginated consistent snapshot. Pages are isolated in staging tables.
Completion atomically replaces the account's accepted Cloud baseline, compares incoming entries
with pending local candidates, and advances to the snapshot resume cursor. Interrupted downloads
remain resumable; interrupted apply work rolls back as one SQLite transaction.

## Error and privacy boundary

Transport errors are classified into offline, authentication required, device revoked, cursor
expired, snapshot expired, rate limited, and fatal outcomes. State stores stable error codes only;
payloads and Bearer tokens are never written to diagnostics or telemetry.
