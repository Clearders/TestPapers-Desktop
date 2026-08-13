-- CLE-89: accepted Cloud baseline and restart-safe delivery bookkeeping.

CREATE TABLE sync_remote_entities (
    account_id TEXT NOT NULL,
    entity_type TEXT NOT NULL
        CHECK (entity_type IN ('question', 'paper', 'draft', 'attachment', 'comment', 'favorite', 'setting')),
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    tombstone INTEGER NOT NULL DEFAULT 0 CHECK (tombstone IN (0, 1)),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    last_sequence TEXT NOT NULL CHECK (last_sequence <> ''),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, entity_type, entity_id)
);
CREATE INDEX sync_remote_entities_sequence_idx
    ON sync_remote_entities(account_id, last_sequence);

ALTER TABLE sync_snapshot_entries ADD COLUMN sequence TEXT;
ALTER TABLE sync_snapshot_entries ADD COLUMN updated_at INTEGER;

