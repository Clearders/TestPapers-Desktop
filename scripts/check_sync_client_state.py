from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "contracts" / "sync-client-state.schema.json"
FIXTURES_PATH = ROOT / "contracts" / "sync-client-state.fixtures.json"
LOCK_PATH = ROOT / "contracts" / "sync-client-state.lock.json"

STATUSES = [
    "synced", "pending", "syncing", "offline", "retrying", "conflict", "authRequired", "failed"
]
SNAPSHOT_KEYS = {
    "schemaVersion", "protocolVersion", "accountId", "deviceId", "status", "paused", "phase",
    "pendingCount", "retryingCount", "conflictCount", "failedCount", "lastCompletedAt",
    "lastErrorCode", "recommendedAction", "canPause", "canResume", "canSyncNow", "canRetry", "entities",
}


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def main() -> int:
    schema_bytes = SCHEMA_PATH.read_bytes()
    fixtures_bytes = FIXTURES_PATH.read_bytes()
    schema = json.loads(schema_bytes)
    fixtures = json.loads(fixtures_bytes)
    lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    schema_hash = digest(schema_bytes)
    fixtures_hash = digest(fixtures_bytes)
    fingerprint = digest(f"{schema_hash}:{fixtures_hash}".encode())

    assert lock["schemaVersion"] == fixtures["schemaVersion"] == 1
    assert lock["protocolVersion"] == fixtures["protocolVersion"] == 1
    assert lock["schemaSha256"] == schema_hash
    assert lock["fixturesSha256"] == fixtures_hash
    assert lock["semanticFingerprint"] == fingerprint
    assert schema["$defs"]["status"]["enum"] == STATUSES
    assert [state["status"] for state in fixtures["states"]] == STATUSES

    for state in fixtures["states"]:
        assert set(state) == SNAPSHOT_KEYS
        assert state["recommendedAction"] in schema["$defs"]["recommendedAction"]["enum"]
        assert "accessToken" not in state and "payload" not in state
        for entity in state["entities"]:
            assert set(entity) == {"entityType", "entityId", "status"}
            assert entity["status"] in STATUSES

    event = fixtures["event"]
    assert set(event) == {"schemaVersion", "type", "occurredAt", "state"}
    assert event["type"] == "sync.statusChanged"
    assert set(event["state"]) == SNAPSHOT_KEYS
    print(f"Sync client state contract verified ({fingerprint}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
