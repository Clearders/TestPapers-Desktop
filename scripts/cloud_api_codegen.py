from __future__ import annotations

import difflib
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "contracts" / "openapi.json"
LOCK = ROOT / "contracts" / "contract.lock.json"
CONFIG = ROOT / "contracts" / "openapi-generator-config.json"
CRATE = ROOT / "contracts" / "cloud-api-rust"
CACHE = ROOT / ".cache" / "openapi-generator"

EXPECTED_REPOSITORY = "https://github.com/Clearders/TestPaper-backend"
EXPECTED_GENERATOR = "openapi-generator-cli"
EXPECTED_GENERATOR_VERSION = "7.24.0"
EXPECTED_GENERATOR_SHA256 = (
    "4b83ccc6fd43056c8c631cd0195e5100bd0550912502527bab09ac76152dab0c"
)
EXPECTED_RUST_VERSION = "1.94.1"
EXPECTED_CONFIG = {
    "generatorName": "rust",
    "additionalProperties": {
        "hideGenerationTimestamp": True,
        "library": "reqwest",
        "packageName": "testpapers-cloud-api",
        "packageVersion": "1.2.0",
        "supportAsync": True,
        "useSingleRequestParameter": True,
    },
    "globalProperties": {
        "apiTests": False,
        "modelTests": False,
    },
}

GENERATED_FILES = (Path("Cargo.toml"), Path("README.md"), Path("src/lib.rs"))
GENERATED_DIRECTORIES = (Path("docs"), Path("src/apis"), Path("src/models"))

NESTED_MODELS_DEFECT = "models::models::"
UNTYPED_ANY_OF_DEFECT = "models::AnyOfLessThanGreaterThan"
EXPECTED_NESTED_MODELS_DEFECTS = 8
EXPECTED_UNTYPED_ANY_OF_DEFECTS = 1


class ContractCodegenError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ContractCodegenError(f"cannot read {path.relative_to(ROOT)}: {error}") from error
    if not isinstance(value, dict):
        raise ContractCodegenError(f"{path.relative_to(ROOT)} must contain a JSON object")
    return value


def validate_contract_lock() -> dict:
    contract = read_json(CONTRACT)
    lock = read_json(LOCK)
    config = read_json(CONFIG)

    api_version = contract.get("info", {}).get("version")
    if lock.get("apiVersion") != api_version:
        raise ContractCodegenError(
            f"contract lock apiVersion {lock.get('apiVersion')!r} does not match "
            f"OpenAPI info.version {api_version!r}"
        )

    source = lock.get("source", {})
    if source.get("repository") != EXPECTED_REPOSITORY:
        raise ContractCodegenError(f"source.repository must be {EXPECTED_REPOSITORY}")
    if source.get("ref") != f"api-v{api_version}":
        raise ContractCodegenError(f"source.ref must be api-v{api_version}")
    if not re.fullmatch(r"[0-9a-f]{40}", str(source.get("commit", ""))):
        raise ContractCodegenError("source.commit must be a lowercase 40-character Git commit SHA")

    actual_contract_sha256 = sha256(CONTRACT)
    if source.get("sha256") != actual_contract_sha256:
        raise ContractCodegenError(
            "source.sha256 is stale: "
            f"expected {actual_contract_sha256}, received {source.get('sha256', 'missing')}"
        )

    generator = lock.get("generator", {})
    if generator.get("name") != EXPECTED_GENERATOR:
        raise ContractCodegenError(f"generator.name must be {EXPECTED_GENERATOR}")
    if generator.get("version") != EXPECTED_GENERATOR_VERSION:
        raise ContractCodegenError(
            f"generator.version must be exactly {EXPECTED_GENERATOR_VERSION}"
        )
    if generator.get("artifactSha256") != EXPECTED_GENERATOR_SHA256:
        raise ContractCodegenError(
            f"generator.artifactSha256 must be {EXPECTED_GENERATOR_SHA256}"
        )
    if config != EXPECTED_CONFIG:
        raise ContractCodegenError(
            f"{CONFIG.relative_to(ROOT)} does not match the pinned Rust/reqwest configuration"
        )
    if generator.get("config") != config:
        raise ContractCodegenError("generator.config must match openapi-generator-config.json")

    return lock


def generator_jar(lock: dict) -> Path:
    generator = lock["generator"]
    version = generator["version"]
    expected_sha256 = generator["artifactSha256"]
    jar = CACHE / f"openapi-generator-cli-{version}.jar"

    if jar.is_file() and sha256(jar) == expected_sha256:
        return jar
    if jar.exists():
        jar.unlink()

    CACHE.mkdir(parents=True, exist_ok=True)
    url = (
        "https://repo.maven.apache.org/maven2/org/openapitools/"
        f"openapi-generator-cli/{version}/openapi-generator-cli-{version}.jar"
    )
    partial = jar.with_suffix(".jar.part")
    try:
        with urllib.request.urlopen(url, timeout=60) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        if sha256(partial) != expected_sha256:
            raise ContractCodegenError(
                f"downloaded OpenAPI Generator {version} has an unexpected SHA-256"
            )
        partial.replace(jar)
    finally:
        if partial.exists():
            partial.unlink()
    return jar


def java_command() -> str:
    java_home = os.environ.get("JAVA_HOME")
    if java_home:
        candidate = Path(java_home) / "bin" / ("java.exe" if os.name == "nt" else "java")
        if candidate.is_file():
            return str(candidate)
    candidate = shutil.which("java")
    if candidate:
        return candidate
    raise ContractCodegenError("Java 17 or newer is required to run OpenAPI Generator")


def run_generator(output: Path, lock: dict) -> None:
    java = java_command()
    jar = generator_jar(lock)
    version_result = subprocess.run(
        [java, "-jar", str(jar), "version"],
        check=False,
        capture_output=True,
        text=True,
    )
    if version_result.returncode != 0:
        raise ContractCodegenError(
            "OpenAPI Generator could not start; Java 17 or newer is required:\n"
            + version_result.stderr.strip()
        )
    if version_result.stdout.strip() != EXPECTED_GENERATOR_VERSION:
        raise ContractCodegenError(
            f"expected OpenAPI Generator {EXPECTED_GENERATOR_VERSION}, "
            f"received {version_result.stdout.strip()!r}"
        )

    global_properties = ",".join(
        f"{name}={str(value).lower()}"
        for name, value in EXPECTED_CONFIG["globalProperties"].items()
    )
    result = subprocess.run(
        [
            java,
            "-jar",
            str(jar),
            "generate",
            "--input-spec",
            str(CONTRACT),
            "--config",
            str(CONFIG),
            "--output",
            str(output),
            "--global-property",
            global_properties,
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = "\n".join(part for part in (result.stdout, result.stderr) if part).strip()
        raise ContractCodegenError(f"OpenAPI generation failed:\n{detail}")


def format_rust_sources(output: Path) -> None:
    rustc = shutil.which("rustc")
    rustfmt = shutil.which("rustfmt")
    if not rustc or not rustfmt:
        raise ContractCodegenError(
            f"Rust {EXPECTED_RUST_VERSION} with rustfmt is required for deterministic generation"
        )
    version = subprocess.run(
        [rustc, "--version"], check=False, capture_output=True, text=True
    )
    if version.returncode != 0 or not version.stdout.startswith(
        f"rustc {EXPECTED_RUST_VERSION} "
    ):
        raise ContractCodegenError(
            f"rustc must be exactly {EXPECTED_RUST_VERSION}; received {version.stdout.strip()!r}"
        )

    rust_files = sorted(str(path) for path in output.rglob("*.rs"))
    for offset in range(0, len(rust_files), 25):
        result = subprocess.run(
            [rustfmt, "--edition", "2021", *rust_files[offset : offset + 25]],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            detail = "\n".join(
                part for part in (result.stdout, result.stderr) if part
            ).strip()
            raise ContractCodegenError(f"rustfmt failed:\n{detail}")


def generated_paths(root: Path) -> set[Path]:
    paths = {path for path in GENERATED_FILES if (root / path).is_file()}
    for directory in GENERATED_DIRECTORIES:
        candidate = root / directory
        if candidate.is_dir():
            paths.update(path.relative_to(root) for path in candidate.rglob("*") if path.is_file())
    return paths


def normalize_and_patch(output: Path) -> None:
    rust_files = sorted(output.rglob("*.rs"))
    nested_count = 0
    any_of_count = 0
    for path in rust_files:
        content = path.read_text(encoding="utf-8")
        nested_count += content.count(NESTED_MODELS_DEFECT)
        any_of_count += content.count(UNTYPED_ANY_OF_DEFECT)

    if nested_count != EXPECTED_NESTED_MODELS_DEFECTS:
        raise ContractCodegenError(
            "OpenAPI Generator compatibility assertion failed: expected exactly "
            f"{EXPECTED_NESTED_MODELS_DEFECTS} nested model paths, found {nested_count}"
        )
    if any_of_count != EXPECTED_UNTYPED_ANY_OF_DEFECTS:
        raise ContractCodegenError(
            "OpenAPI Generator compatibility assertion failed: expected exactly "
            f"{EXPECTED_UNTYPED_ANY_OF_DEFECTS} unconstrained anyOf references, "
            f"found {any_of_count}"
        )

    for path in rust_files:
        content = path.read_text(encoding="utf-8")
        content = content.replace(NESTED_MODELS_DEFECT, "models::")
        content = content.replace(UNTYPED_ANY_OF_DEFECT, "serde_json::Value")
        # OpenAPI Generator emits Literal[true|false] as a string enum, which cannot decode the
        # API's actual JSON boolean envelope field. Keep the generated field optional but typed as
        # bool so every success/error envelope is wire compatible.
        content = content.replace("pub success: Option<Success>,", "pub success: Option<bool>,")
        path.write_text(content, encoding="utf-8", newline="\n")

    cargo = output / "Cargo.toml"
    cargo_content = cargo.read_text(encoding="utf-8")
    if "[dev-dependencies]" in cargo_content:
        raise ContractCodegenError("generated Cargo.toml unexpectedly contains dev-dependencies")
    cargo_content = cargo_content.rstrip() + (
        "\n\n[dev-dependencies]\n"
        'tokio = { version = "^1.0", features = ["macros", "rt-multi-thread"] }\n'
    )
    cargo.write_text(cargo_content, encoding="utf-8", newline="\n")

    format_rust_sources(output)

    lib = output / "src" / "lib.rs"
    lib_content = lib.read_text(encoding="utf-8")
    hook = "pub mod models;"
    if lib_content.count(hook) != 1 or "pub mod adapter;" in lib_content:
        raise ContractCodegenError("generated src/lib.rs no longer matches the adapter hook")
    lib_content = lib_content.replace(
        hook,
        f"{hook}\n\n// Handwritten native-client boundary; not emitted by OpenAPI Generator.\n"
        "pub mod adapter;",
    )
    lib_content = lib_content.replace(
        "#![allow(clippy::too_many_arguments)]",
        "#![allow(clippy::too_many_arguments)]\n#![allow(non_snake_case)]",
    )
    lib.write_text(lib_content, encoding="utf-8", newline="\n")

    for path in generated_paths(output):
        candidate = output / path
        try:
            content = candidate.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            raise ContractCodegenError(f"generated file is not UTF-8: {path}") from error
        lines = [line.rstrip() for line in content.splitlines()]
        while lines and not lines[-1]:
            lines.pop()
        content = "\n".join(lines) + "\n"
        candidate.write_text(content, encoding="utf-8", newline="\n")


def generate_expected(output: Path, lock: dict) -> set[Path]:
    run_generator(output, lock)
    normalize_and_patch(output)
    paths = generated_paths(output)
    if not all(path in paths for path in GENERATED_FILES):
        raise ContractCodegenError("OpenAPI Generator omitted a required crate file")
    return paths


def regenerate(output: Path, expected: set[Path]) -> None:
    CRATE.mkdir(parents=True, exist_ok=True)
    existing = generated_paths(CRATE)

    for relative in sorted(existing - expected, reverse=True):
        (CRATE / relative).unlink()
    for relative in sorted(expected):
        destination = CRATE / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output / relative, destination)

    for directory in sorted(GENERATED_DIRECTORIES, reverse=True):
        candidate = CRATE / directory
        if candidate.is_dir():
            for child in sorted(candidate.rglob("*"), reverse=True):
                if child.is_dir() and not any(child.iterdir()):
                    child.rmdir()


def check_drift(output: Path, expected: set[Path]) -> bool:
    actual = generated_paths(CRATE)
    problems: list[str] = []

    for relative in sorted(expected - actual):
        problems.append(f"missing: {relative.as_posix()}")
    for relative in sorted(actual - expected):
        problems.append(f"stale: {relative.as_posix()}")
    for relative in sorted(expected & actual):
        expected_bytes = (output / relative).read_bytes()
        actual_bytes = (CRATE / relative).read_bytes()
        if expected_bytes != actual_bytes:
            problems.append(f"changed: {relative.as_posix()}")
            if len(problems) == 1:
                expected_text = expected_bytes.decode("utf-8").splitlines()
                actual_text = actual_bytes.decode("utf-8").splitlines()
                diff = difflib.unified_diff(
                    actual_text,
                    expected_text,
                    fromfile=f"committed/{relative.as_posix()}",
                    tofile=f"generated/{relative.as_posix()}",
                    lineterm="",
                )
                problems.extend(f"  {line}" for line in list(diff)[:80])

    if problems:
        print("Generated Cloud API contract drift detected:", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        print(
            "Run: python scripts/regenerate_cloud_api_rust.py",
            file=sys.stderr,
        )
        return False
    return True


def run(mode: str) -> int:
    try:
        lock = validate_contract_lock()
        with tempfile.TemporaryDirectory(prefix="testpapers-cloud-api-") as temporary:
            output = Path(temporary)
            expected = generate_expected(output, lock)
            if mode == "regenerate":
                regenerate(output, expected)
                print(
                    f"Regenerated {len(expected)} Cloud API contract files with "
                    f"OpenAPI Generator {EXPECTED_GENERATOR_VERSION}."
                )
                return 0
            if mode == "check":
                if not check_drift(output, expected):
                    return 1
                print(
                    f"Cloud API contract is current ({lock['source']['sha256']})."
                )
                return 0
            raise ContractCodegenError(f"unknown mode: {mode}")
    except ContractCodegenError as error:
        print(f"Cloud API contract generation failed: {error}", file=sys.stderr)
        return 1
