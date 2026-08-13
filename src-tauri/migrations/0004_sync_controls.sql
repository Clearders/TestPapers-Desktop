-- CLE-97: user-controlled, restart-safe Sync v1 pause state.

ALTER TABLE sync_runtime_state ADD COLUMN paused INTEGER NOT NULL DEFAULT 0
    CHECK (paused IN (0, 1));
