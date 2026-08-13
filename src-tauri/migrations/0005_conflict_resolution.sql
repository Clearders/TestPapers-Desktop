ALTER TABLE sync_conflict_baselines ADD COLUMN account_id TEXT;
ALTER TABLE sync_conflict_baselines ADD COLUMN reason TEXT;
ALTER TABLE sync_conflict_baselines ADD COLUMN hydration_state TEXT NOT NULL DEFAULT 'complete'
    CHECK (hydration_state IN ('placeholder', 'complete'));

CREATE TABLE sync_conflict_resolutions (
    resolution_id TEXT PRIMARY KEY,
    conflict_id TEXT NOT NULL,
    operation_id TEXT NOT NULL UNIQUE,
    request_hash TEXT NOT NULL CHECK (length(request_hash) = 64 AND request_hash = lower(request_hash)),
    action TEXT NOT NULL
        CHECK (action IN ('keepLocal', 'useCloud', 'saveCopy', 'manualMerge', 'restoreVersion', 'undo')),
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    response_json TEXT CHECK (response_json IS NULL OR json_valid(response_json)),
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'in_flight', 'accepted', 'failed')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (conflict_id) REFERENCES sync_conflict_baselines(conflict_id) ON DELETE RESTRICT
);
CREATE INDEX sync_conflict_resolutions_conflict_idx
    ON sync_conflict_resolutions(conflict_id, created_at DESC);
