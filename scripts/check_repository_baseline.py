from __future__ import annotations

import argparse
import fnmatch
import re
import sys
from pathlib import Path

CANONICAL_ADR = "https://github.com/Clearders/TestPapers/blob/main/docs/adr/0001-platform-repository-and-runtime-boundaries.md"
REPOSITORIES = {
    "TestPapers-Desktop": {
        "runtime": "Runtime owner: Desktop team.",
        "release": "Release unit: signed Desktop installer and its bundled Rust Local Engine.",
        "implementation_issue": "CLE-23",
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
        description="Validate the code-neutral TestPapers repository governance baseline."
    )
    parser.add_argument("--repository", required=True, choices=sorted(REPOSITORIES))
    return parser.parse_args()


def read_utf8(path: Path, errors: list[str]) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        errors.append(f"{path.as_posix()} is not valid UTF-8")
        return ""


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
        if ".git" in path.parts:
            continue
        relative = path.relative_to(root)
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
