from __future__ import annotations

import argparse
import fnmatch
import json
import re
import sys
import tomllib
from pathlib import Path

CANONICAL_ADR = "https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md"
REPOSITORIES = {
    "TestPapers-Desktop": {
        "runtime": "Runtime owner: Desktop team.",
        "release": "Release unit: signed Desktop installer and its bundled Rust Local Engine.",
        "implementation_issue": "CLE-24",
    },
    "TestPapers-Mobile": {
        "runtime": "Runtime owner: Mobile team.",
        "release": "Release unit: signed Android and iOS applications.",
        "implementation_issue": "CLE-35",
    },
}
REQUIRED_FILES = (
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".github/CODEOWNERS",
    ".github/pull_request_template.md",
    ".github/workflows/repository-baseline.yml",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.md",
    "SECURITY.md",
    "scripts/check_repository_baseline.py",
)
DESKTOP_CONTRACT_FILES = (
    "contracts/contract.lock.json",
    "contracts/openapi-generator-config.json",
    "contracts/openapi.json",
    "contracts/cloud-api-rust/Cargo.toml",
    "contracts/cloud-api-rust/src/adapter.rs",
    "contracts/cloud-api-rust/tests/adapter.rs",
    "scripts/check_cloud_api_rust.py",
    "scripts/cloud_api_codegen.py",
    "scripts/regenerate_cloud_api_rust.py",
)
DESKTOP_APP_FILES = (
    ".node-version",
    "package.json",
    "package-lock.json",
    "vite.config.ts",
    "src/App.vue",
    "src/application/useDesktopShell.ts",
    "src/infrastructure/tauri/shellBridge.ts",
    "src/types/shell.ts",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "src-tauri/tauri.conf.json",
    "src-tauri/capabilities/main-window.json",
    "src-tauri/src/application/mod.rs",
    "src-tauri/src/domain/mod.rs",
    "src-tauri/src/infrastructure/mod.rs",
    "src-tauri/src/ipc/mod.rs",
    "src-tauri/src/sync/mod.rs",
    "src-tauri/migrations/README.md",
)
DESKTOP_ALLOWED_MANIFESTS = {
    "package.json",
    "src-tauri/Cargo.toml",
    "contracts/cloud-api-rust/Cargo.toml",
}
DESKTOP_COMMANDS = {
    "get_shell_context",
    "frontend_ready",
    "set_theme_preference",
    "set_close_behavior",
    "resolve_close_request",
    "preview_question_import_dialog",
    "preview_paper_export_dialog",
}
DESKTOP_EVENT_PERMISSIONS = {"core:event:allow-listen", "core:event:allow-unlisten"}
DESKTOP_FORBIDDEN_NPM_DEPENDENCIES = {
    "nuxt",
    "@tauri-apps/plugin-fs",
    "@tauri-apps/plugin-http",
    "@tauri-apps/plugin-shell",
    "@tauri-apps/plugin-sql",
}
DESKTOP_FORBIDDEN_CARGO_DEPENDENCIES = {
    "reqwest",
    "sqlx",
    "tauri-plugin-fs",
    "tauri-plugin-http",
    "tauri-plugin-shell",
    "tauri-plugin-sql",
}
IGNORED_DIRECTORY_NAMES = {".git", ".cache", "node_modules", "target"}
MANIFEST_NAMES = {
    "Cargo.toml",
    "package.json",
    "pnpm-workspace.yaml",
    "pubspec.yaml",
    "pyproject.toml",
}
SECRET_PATTERNS = (
    ".env",
    ".env.*",
    "*.jks",
    "*.key",
    "*.keystore",
    "*.mobileprovision",
    "*.p12",
    "*.p8",
    "*.pem",
    "*.pfx",
    "GoogleService-Info.plist",
    "google-services.json",
    "keystore.properties",
)
FORBIDDEN_REPOSITORIES = (
    "TestPapers",
    "TestPaper-backend",
    "TestPapers-Desktop",
    "TestPapers-Mobile",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the TestPapers repository governance and contract baseline."
    )
    parser.add_argument("--repository", required=True, choices=sorted(REPOSITORIES))
    return parser.parse_args()


def read_utf8(path: Path, errors: list[str]) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        errors.append(f"{path.as_posix()} is not valid UTF-8")
        return ""
    except OSError as error:
        errors.append(f"could not read {path.as_posix()}: {error}")
        return ""


def load_json(path: Path, errors: list[str]) -> dict:
    try:
        value = json.loads(read_utf8(path, errors))
    except json.JSONDecodeError as error:
        errors.append(f"{path.as_posix()} is not valid JSON: {error}")
        return {}
    if not isinstance(value, dict):
        errors.append(f"{path.as_posix()} must contain a JSON object")
        return {}
    return value


def validate_desktop_shell(root: Path, errors: list[str]) -> None:
    for relative in DESKTOP_APP_FILES:
        candidate = root / relative
        if not candidate.is_file() or candidate.stat().st_size == 0:
            errors.append(f"missing or empty Desktop shell file: {relative}")

    package = load_json(root / "package.json", errors)
    all_dependencies = {
        **package.get("dependencies", {}),
        **package.get("devDependencies", {}),
    }
    forbidden_npm = DESKTOP_FORBIDDEN_NPM_DEPENDENCIES & all_dependencies.keys()
    if forbidden_npm:
        errors.append(
            "Desktop shell has forbidden npm dependencies: "
            + ", ".join(sorted(forbidden_npm))
        )

    try:
        cargo = tomllib.loads(read_utf8(root / "src-tauri/Cargo.toml", errors))
    except tomllib.TOMLDecodeError as error:
        errors.append(f"src-tauri/Cargo.toml is not valid TOML: {error}")
        cargo = {}
    cargo_dependencies = set(cargo.get("dependencies", {}))
    forbidden_cargo = DESKTOP_FORBIDDEN_CARGO_DEPENDENCIES & cargo_dependencies
    if forbidden_cargo:
        errors.append(
            "Desktop shell has forbidden Cargo dependencies: "
            + ", ".join(sorted(forbidden_cargo))
        )

    tauri_config = load_json(root / "src-tauri/tauri.conf.json", errors)
    app_config = tauri_config.get("app", {})
    windows = app_config.get("windows", [])
    if app_config.get("withGlobalTauri") is not False:
        errors.append("Tauri global API must remain disabled")
    if len(windows) != 1 or windows[0].get("label") != "main":
        errors.append("Tauri must define exactly one main window")
    elif windows[0].get("visible") is not False:
        errors.append("The main window must stay hidden until frontend_ready")
    dev_url = tauri_config.get("build", {}).get("devUrl", "")
    if dev_url and not re.fullmatch(r"http://(?:localhost|127\.0\.0\.1):\d+", dev_url):
        errors.append("Remote Tauri development URLs are forbidden")

    capability = load_json(root / "src-tauri/capabilities/main-window.json", errors)
    permissions = set(capability.get("permissions", []))
    expected_permissions = DESKTOP_EVENT_PERMISSIONS | {
        f"allow-{command.replace('_', '-')}" for command in DESKTOP_COMMANDS
    }
    if capability.get("local") is not True:
        errors.append("The main-window capability must be local-only")
    if capability.get("windows") != ["main"]:
        errors.append("The main-window capability must target only the main window")
    if permissions != expected_permissions:
        errors.append("The main-window capability permissions differ from the narrow CLE-23 allowlist")
    for permission in permissions:
        if permission == "core:default" or permission.startswith(("fs:", "http:", "shell:", "sql:")):
            errors.append(f"forbidden broad Tauri permission: {permission}")

    bridges = {
        (root / "src/infrastructure/tauri/shellBridge.ts").resolve(),
        (root / "src/infrastructure/tauri/localEngineBridge.ts").resolve(),
    }
    for path in (root / "src").rglob("*"):
        if path.is_file() and path.suffix in {".ts", ".vue"} and path.resolve() not in bridges:
            if re.search(r"\binvoke\s*\(|@tauri-apps/api/(?:core|event)", read_utf8(path, errors)):
                errors.append(
                    "Vue application code bypasses the typed Tauri bridge: "
                    f"{path.relative_to(root).as_posix()}"
                )

    build_script = read_utf8(root / "src-tauri/build.rs", errors)
    declared_commands = set(re.findall(r'"([a-z][a-z0-9_]+)"', build_script))
    if declared_commands != DESKTOP_COMMANDS:
        errors.append("src-tauri/build.rs command manifest differs from the CLE-23 allowlist")

    rust_source = "\n".join(
        read_utf8(path, errors) for path in (root / "src-tauri/src").rglob("*.rs")
    )
    for forbidden in ("contracts/cloud-api-rust", "reqwest::", "sqlx::"):
        if forbidden in rust_source:
            errors.append(f"Desktop shell source starts a deferred integration: {forbidden}")


def matches_secret(path: Path) -> bool:
    if path.name == ".env.example":
        return False
    return any(fnmatch.fnmatch(path.name, pattern) for pattern in SECRET_PATTERNS)


def validate(repository: str, root: Path) -> list[str]:
    errors: list[str] = []
    expected = REPOSITORIES[repository]

    for relative in REQUIRED_FILES:
        candidate = root / relative
        if not candidate.is_file() or candidate.stat().st_size == 0:
            errors.append(f"missing or empty required file: {relative}")

    if repository == "TestPapers-Desktop":
        for relative in DESKTOP_CONTRACT_FILES:
            candidate = root / relative
            if not candidate.is_file() or candidate.stat().st_size == 0:
                errors.append(f"missing or empty Desktop contract file: {relative}")
        validate_desktop_shell(root, errors)

    readme = read_utf8(root / "README.md", errors)
    required_readme_tokens = (
        f"# {repository.replace('-', ' ')}",
        "CLE-58",
        expected["runtime"],
        expected["release"],
        expected["implementation_issue"],
        CANONICAL_ADR,
        "Do not add source-level relative-path dependencies",
    )
    for token in required_readme_tokens:
        if token not in readme:
            errors.append(f"README.md is missing required text: {token}")

    template = read_utf8(root / ".github/pull_request_template.md", errors)
    for heading in (
        "## Linear",
        "## Scope",
        "## Validation",
        "## Migration and rollback",
    ):
        if heading not in template:
            errors.append(f"pull request template is missing heading: {heading}")
    if "## Security" not in template:
        errors.append("pull request template is missing a Security heading")

    codeowners = read_utf8(root / ".github/CODEOWNERS", errors)
    if "@Clearders" not in codeowners:
        errors.append("CODEOWNERS must include the repository owner")

    relative_dependency = re.compile(
        rf"(?:file:|path\s*[:=]\s*[\"']?)\.\.[/\\](?:{'|'.join(map(re.escape, FORBIDDEN_REPOSITORIES))})(?:[/\\]|[\"'])",
        re.IGNORECASE,
    )
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if any(part in IGNORED_DIRECTORY_NAMES for part in relative.parts):
            continue
        if path.is_symlink() and not path.resolve().is_relative_to(root.resolve()):
            errors.append(f"external symlink is forbidden: {relative.as_posix()}")
        if path.is_file() and matches_secret(path):
            errors.append(
                f"credential or secret file is forbidden: {relative.as_posix()}"
            )
        if path.is_file() and path.name in MANIFEST_NAMES:
            content = read_utf8(path, errors)
            if relative_dependency.search(content):
                errors.append(
                    f"cross-repository relative dependency is forbidden: {relative.as_posix()}"
                )
            if repository == "TestPapers-Desktop" and relative.as_posix() not in DESKTOP_ALLOWED_MANIFESTS:
                errors.append(
                    f"unexpected Desktop manifest: {relative.as_posix()}"
                )

    return errors


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    errors = validate(args.repository, root)
    if errors:
        print("Repository baseline validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(f"Repository baseline validation passed for {args.repository}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
