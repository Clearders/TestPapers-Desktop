import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "check_local_data_contract.py"
SPEC = importlib.util.spec_from_file_location("check_local_data_contract", SCRIPT)
assert SPEC and SPEC.loader
contract = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(contract)


class LocalDataContractTests(unittest.TestCase):
    def test_repository_projection_is_complete(self):
        self.assertEqual(contract.validate(), [])

    def test_missing_entity_table_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "contracts").mkdir()
            (root / "src-tauri" / "migrations").mkdir(parents=True)
            (root / "contracts" / "domain-model.lock.json").write_text(
                '{"version":"1.0.0","status":"accepted","linearIssue":"CLE-15","sha256":"' + "a" * 64 + '"}',
                encoding="utf-8",
            )
            (root / "src-tauri" / "migrations" / "0001_local_data.sql").write_text(
                "PRAGMA foreign_keys = ON; PRAGMA user_version = 1; CREATE VIRTUAL TABLE questions_fts USING fts5(text);",
                encoding="utf-8",
            )
            self.assertTrue(any("workspace_meta" in error for error in contract.validate(root)))


if __name__ == "__main__":
    unittest.main()
