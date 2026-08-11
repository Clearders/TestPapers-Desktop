#!/usr/bin/env python3
"""Validate the pinned CLE-15 local model projection without another checkout."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = ROOT / "contracts" / "domain-model.lock.json"
MIGRATION_PATH = ROOT / "src-tauri" / "migrations" / "0001_local_data.sql"

REQUIRED_TABLES = {
    "workspace_meta",
    "questions",
    "question_subjects",
    "question_tags",
    "entity_history",
    "pending_mutations",
    "papers",
    "paper_items",
    "drafts",
    "attachments",
    "comments",
    "favorites",
    "settings",
}


def validate(root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    lock_path = root / LOCK_PATH.relative_to(ROOT)
    migration_path = root / MIGRATION_PATH.relative_to(ROOT)
    try:
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read domain model lock: {error}"]

    if lock.get("version") != "1.0.0" or lock.get("status") != "accepted":
        errors.append("domain model lock must pin accepted version 1.0.0")
    if not re.fullmatch(r"[0-9a-f]{64}", str(lock.get("sha256", ""))):
        errors.append("domain model lock sha256 must be lowercase hexadecimal")
    if lock.get("linearIssue") != "CLE-15":
        errors.append("domain model lock must identify CLE-15")

    try:
        migration = migration_path.read_text(encoding="utf-8").lower()
    except OSError as error:
        return errors + [f"cannot read initial local migration: {error}"]

    declared = set(re.findall(r"create\s+(?:virtual\s+)?table\s+(?:if\s+not\s+exists\s+)?([a-z_]+)", migration))
    for table in sorted(REQUIRED_TABLES - declared):
        errors.append(f"initial local migration is missing {table}")
    for pragma in ("foreign_keys", "user_version"):
        if pragma not in migration:
            errors.append(f"initial local migration is missing {pragma} declaration")
    if "using fts5" not in migration:
        errors.append("initial local migration must create an FTS5 index")
    return errors


def main() -> int:
    errors = validate()
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print("Local data contract is valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
