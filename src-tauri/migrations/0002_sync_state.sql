-- CLE-91: persistent Sync v1 delivery, cursor, conflict, and snapshot-rebuild state.
-- This migration is additive so a v1 rollback copy remains a usable downgrade path.

ALTER TABLE pending_mutations ADD COLUMN account_id TEXT;
ALTER TABLE pending_mutations ADD COLUMN device_id TEXT;
ALTER TABLE pending_mutations ADD COLUMN batch_id TEXT;
ALTER TABLE pending_mutations ADD COLUMN batch_ordinal INTEGER NOT NULL DEFAULT 0
    CHECK (batch_ordinal >= 0);
ALTER TABLE pending_mutations ADD COLUMN queue_state TEXT NOT NULL DEFAULT 'pending'
    CHECK (queue_state IN ('pending', 'in_flight', 'retrying', 'conflict', 'failed', 'settled'));
ALTER TABLE pending_mutations ADD COLUMN dependencies_json TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(dependencies_json) AND json_type(dependencies_json) = 'array');
ALTER TABLE pending_mutations ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0
    CHECK (attempt_count >= 0);
ALTER TABLE pending_mutations ADD COLUMN next_attempt_at INTEGER;
ALTER TABLE pending_mutations ADD COLUMN last_attempt_at INTEGER;
ALTER TABLE pending_mutations ADD COLUMN last_error_code TEXT;
ALTER TABLE pending_mutations ADD COLUMN request_hash TEXT
    CHECK (request_hash IS NULL OR (length(request_hash) = 64 AND request_hash = lower(request_hash)));
ALTER TABLE pending_mutations ADD COLUMN stored_response_json TEXT
    CHECK (stored_response_json IS NULL OR json_valid(stored_response_json));
ALTER TABLE pending_mutations ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;

UPDATE pending_mutations SET updated_at = created_at WHERE updated_at = 0;

CREATE INDEX pending_mutations_delivery_idx
    ON pending_mutations(queue_state, next_attempt_at, created_at, operation_id);
CREATE INDEX pending_mutations_device_idx
    ON pending_mutations(account_id, device_id, queue_state, created_at, operation_id);

CREATE TABLE sync_devices (
    account_id TEXT NOT NULL CHECK (length(account_id) = 36 AND account_id = lower(account_id)),
    device_id TEXT NOT NULL CHECK (length(device_id) = 36 AND device_id = lower(device_id)),
    protocol_version TEXT NOT NULL DEFAULT 'v1' CHECK (protocol_version = 'v1'),
    acknowledged_cursor TEXT,
    pulled_cursor TEXT,
    authentication_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (authentication_state IN ('ready', 'required', 'revoked')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, device_id)
);

CREATE TABLE sync_runtime_state (
    account_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    phase TEXT NOT NULL DEFAULT 'idle'
        CHECK (phase IN ('idle', 'pull', 'apply', 'ack', 'push', 'settle')),
    active_batch_id TEXT,
    phase_started_at INTEGER,
    last_completed_at INTEGER,
    last_error_code TEXT,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, device_id),
    FOREIGN KEY (account_id, device_id)
        REFERENCES sync_devices(account_id, device_id) ON DELETE CASCADE
);

CREATE TABLE sync_operation_dependencies (
    operation_id TEXT NOT NULL REFERENCES pending_mutations(operation_id) ON DELETE CASCADE,
    depends_on_operation_id TEXT NOT NULL,
    PRIMARY KEY (operation_id, depends_on_operation_id),
    CHECK (operation_id <> depends_on_operation_id)
);
CREATE INDEX sync_operation_dependencies_target_idx
    ON sync_operation_dependencies(depends_on_operation_id, operation_id);

-- Results deliberately outlive pending queue cleanup so a committed response can be replayed
-- after a dropped transport response or process restart.
CREATE TABLE sync_operation_results (
    operation_id TEXT PRIMARY KEY,
    request_hash TEXT NOT NULL CHECK (length(request_hash) = 64 AND request_hash = lower(request_hash)),
    response_json TEXT NOT NULL CHECK (json_valid(response_json)),
    recorded_at INTEGER NOT NULL
);

CREATE TABLE sync_conflict_baselines (
    conflict_id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation_id TEXT,
    base_version INTEGER,
    base_content_hash TEXT,
    base_snapshot_json TEXT CHECK (base_snapshot_json IS NULL OR json_valid(base_snapshot_json)),
    local_snapshot_json TEXT NOT NULL CHECK (json_valid(local_snapshot_json)),
    cloud_version INTEGER NOT NULL CHECK (cloud_version >= 1),
    cloud_content_hash TEXT NOT NULL CHECK (length(cloud_content_hash) = 64),
    cloud_snapshot_json TEXT NOT NULL CHECK (json_valid(cloud_snapshot_json)),
    resolution_state TEXT NOT NULL DEFAULT 'unresolved'
        CHECK (resolution_state IN ('unresolved', 'resolving', 'resolved', 'undone')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK ((base_version IS NULL AND base_content_hash IS NULL)
        OR (base_version IS NOT NULL AND base_version >= 1
            AND base_content_hash IS NOT NULL AND length(base_content_hash) = 64))
);
CREATE INDEX sync_conflict_baselines_entity_idx
    ON sync_conflict_baselines(entity_type, entity_id, created_at DESC);

CREATE TABLE sync_snapshot_rebuilds (
    rebuild_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    snapshot_id TEXT,
    state TEXT NOT NULL DEFAULT 'downloading'
        CHECK (state IN ('downloading', 'ready', 'applying', 'swapping', 'complete', 'failed')),
    resume_cursor TEXT,
    pages_received INTEGER NOT NULL DEFAULT 0 CHECK (pages_received >= 0),
    last_error_code TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (account_id, device_id)
        REFERENCES sync_devices(account_id, device_id) ON DELETE CASCADE
);
CREATE INDEX sync_snapshot_rebuilds_device_idx
    ON sync_snapshot_rebuilds(account_id, device_id, state, updated_at);

CREATE TABLE sync_snapshot_entries (
    rebuild_id TEXT NOT NULL REFERENCES sync_snapshot_rebuilds(rebuild_id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    tombstone INTEGER NOT NULL DEFAULT 0 CHECK (tombstone IN (0, 1)),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    PRIMARY KEY (rebuild_id, entity_type, entity_id)
);
