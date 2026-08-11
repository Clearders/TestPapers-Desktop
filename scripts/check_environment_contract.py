from __future__ import annotations

import argparse
import sys
from pathlib import Path
from urllib.parse import urlparse

PROFILES = {"local", "development", "test", "staging", "production"}
MODES = {"offline", "local", "staging", "production"}
REQUIRED = {
    "TESTPAPERS_ENV",
    "TESTPAPERS_DATA_DIR",
    "TESTPAPERS_SQLITE_PATH",
    "TESTPAPERS_EXPORT_DIR",
    "TESTPAPERS_CLOUD_API_MODE",
    "TESTPAPERS_CLOUD_LOCAL_API_BASE",
    "TESTPAPERS_STAGING_API_BASE",
    "TESTPAPERS_PRODUCTION_API_BASE",
}
FORBIDDEN_PREFIXES = ("DATABASE_URL", "REDIS", "CELERY", "S3_", "MINIO_", "OBJECT_STORE", "AWS_", "AZURE_STORAGE", "GCS_")
SECRET_MARKERS = ("TOKEN", "SECRET", "PASSWORD", "API_KEY", "CREDENTIAL")


def parse_env(path: Path) -> tuple[dict[str, str], list[str]]:
    values: dict[str, str] = {}
    errors: list[str] = []
    if not path.is_file():
        return values, [f"environment file is missing: {path}"]
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            errors.append(f"line {number}: expected KEY=VALUE")
            continue
        key, value = line.split("=", 1)
        if not key or key.strip() != key:
            errors.append(f"line {number}: invalid key")
        elif key in values:
            errors.append(f"line {number}: duplicate key {key}")
        else:
            values[key] = value.strip()
    return values, errors


def valid_url(value: str, require_https: bool) -> bool:
    parsed = urlparse(value)
    return bool(parsed.netloc) and parsed.scheme in ({"https"} if require_https else {"http", "https"}) and parsed.path in ("", "/") and not parsed.username and not parsed.password and not parsed.query and not parsed.fragment


def validate(values: dict[str, str]) -> list[str]:
    errors = [f"missing required setting: {key}" for key in sorted(REQUIRED - values.keys())]
    for key, value in values.items():
        normalized_key = key.upper()
        if normalized_key.startswith(FORBIDDEN_PREFIXES):
            errors.append(f"{key} is forbidden: Desktop never configures Cloud infrastructure")
        if any(marker in normalized_key for marker in SECRET_MARKERS):
            errors.append(f"{key} is forbidden: credentials and secrets must not be environment configuration")
        if not value and key in REQUIRED:
            errors.append(f"{key} must not be blank")
    if values.get("TESTPAPERS_ENV") not in PROFILES:
        errors.append("TESTPAPERS_ENV must be one of: local, development, test, staging, production")
    mode = values.get("TESTPAPERS_CLOUD_API_MODE")
    if mode not in MODES:
        errors.append("TESTPAPERS_CLOUD_API_MODE must be one of: offline, local, staging, production")
    profile = values.get("TESTPAPERS_ENV")
    if profile == "staging" and mode != "staging":
        errors.append("TESTPAPERS_CLOUD_API_MODE must be staging when TESTPAPERS_ENV is staging")
    if profile == "production" and mode != "production":
        errors.append("TESTPAPERS_CLOUD_API_MODE must be production when TESTPAPERS_ENV is production")
    if profile == "test" and mode not in {"offline", "local"}:
        errors.append("TESTPAPERS_CLOUD_API_MODE must be offline or local when TESTPAPERS_ENV is test")
    for key in ("TESTPAPERS_DATA_DIR", "TESTPAPERS_SQLITE_PATH", "TESTPAPERS_EXPORT_DIR"):
        if values.get(key) and values[key].startswith(("http://", "https://")):
            errors.append(f"{key} must be a local filesystem path")
    if mode == "local" and not valid_url(values.get("TESTPAPERS_CLOUD_LOCAL_API_BASE", ""), False):
        errors.append("TESTPAPERS_CLOUD_LOCAL_API_BASE must be a credential-free http(s) Cloud origin when Cloud API mode is local")
    if mode == "staging" and not valid_url(values.get("TESTPAPERS_STAGING_API_BASE", ""), True):
        errors.append("TESTPAPERS_STAGING_API_BASE must be a credential-free HTTPS origin when Cloud API mode is staging")
    if mode == "production" and not valid_url(values.get("TESTPAPERS_PRODUCTION_API_BASE", ""), True):
        errors.append("TESTPAPERS_PRODUCTION_API_BASE must be a credential-free HTTPS origin when Cloud API mode is production")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate the Desktop code-neutral environment contract.")
    parser.add_argument("--env-file", type=Path, default=Path(__file__).resolve().parents[1] / ".env.example")
    args = parser.parse_args()
    values, errors = parse_env(args.env_file)
    errors.extend(validate(values))
    if errors:
        print("Desktop environment contract validation failed:", file=sys.stderr)
        print(*(f"- {error}" for error in errors), sep="\n", file=sys.stderr)
        return 1
    print("Desktop environment contract validation passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
