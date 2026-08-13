from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "contracts" / "sync-v1.schema.json"
FIXTURES_PATH = ROOT / "contracts" / "sync-v1.fixtures.json"
LOCK_PATH = ROOT / "contracts" / "sync-v1.lock.json"


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def main() -> int:
    schema_bytes = SCHEMA_PATH.read_bytes()
    fixture_bytes = FIXTURES_PATH.read_bytes()
    schema = json.loads(schema_bytes)
    fixtures = json.loads(fixture_bytes)
    lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))

    schema_hash = digest(schema_bytes)
    fixture_hash = digest(fixture_bytes)
    fingerprint = digest(f"{schema_hash}:{fixture_hash}".encode())
    assert lock["protocolVersion"] == schema["protocolVersion"] == fixtures["protocolVersion"] == 1
    assert lock["schemaSha256"] == schema_hash
    assert lock["fixturesSha256"] == fixture_hash
    assert lock["semanticFingerprint"] == fingerprint

    for case in fixtures["canonicalCases"]:
        canonical = canonical_json(case["input"])
        assert canonical == case["canonical"], case["name"]
        assert digest(canonical.encode()) == case["sha256"], case["name"]

    schema_errors = schema["$defs"]["errorCode"]["enum"]
    fixture_errors = [case["code"] for case in fixtures["errorCases"]]
    assert schema_errors == fixture_errors
    assert len(set(schema["$defs"]["entityType"]["enum"])) == 7
    stale_delete = next(case for case in fixtures["lifecycleCases"] if case["name"] == "stale-update-after-delete")
    assert stale_delete["thirdStatus"] == "conflict"
    print(f"Sync v1 contract verified ({fingerprint}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
