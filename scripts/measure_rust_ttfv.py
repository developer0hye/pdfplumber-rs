#!/usr/bin/env python3
"""Measure and validate the clean-project Rust time to first value."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import tomllib

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts import check_doc_quickstarts as quick_starts

SCHEMA_VERSION = 1
TTFV_LIMIT_SECONDS = 300
DEFAULT_CANDIDATE_VERSION = "0.3.0"
FIXTURE = ROOT / "tests/fixtures/generated/basic_text.pdf"
CANDIDATE_CRATES = (
    ROOT / "crates/pdfplumber-core",
    ROOT / "crates/pdfplumber-parse",
    ROOT / "crates/pdfplumber",
)
PHASES = (
    "project_creation",
    "dependency_declaration",
    "code_copy",
    "resolve_build_execute",
    "interpretation",
)
COVERAGE = ("installation", "code_copy", "execution", "interpretation")
CARGO_NEW_COMMAND = "cargo new --bin --vcs none --name pdfplumber-ttfv consumer"
CARGO_RUN_COMMAND = "cargo run --quiet"


class MeasurementError(RuntimeError):
    """The TTFV protocol or a recorded result is invalid."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_text(value: str) -> str:
    return sha256_bytes(value.encode("utf-8"))


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def command(
    arguments: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        arguments,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        rendered = " ".join(arguments)
        raise MeasurementError(
            f"command failed ({completed.returncode}) in {cwd}: {rendered}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def version_output(arguments: list[str]) -> str:
    return command(arguments, cwd=ROOT).stdout.strip()


def measurement_inputs() -> tuple[str, str]:
    installation, snippets = quick_starts.surface_snippets("rust")
    if not snippets:
        raise MeasurementError("README.md has no rendered Rust quick start")
    snippet = snippets[0]
    quick_starts.validate_primary_rust_quick_start(snippet)
    if not FIXTURE.is_file():
        raise MeasurementError(f"missing measurement fixture: {FIXTURE}")
    return installation, snippet


def clean_cargo_environment(cargo_home: Path) -> dict[str, str]:
    env = os.environ.copy()
    for variable in (
        "CARGO_BUILD_BUILD_DIR",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_TARGET_DIR",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
    ):
        env.pop(variable, None)
    env["CARGO_HOME"] = str(cargo_home)
    env["CARGO_TERM_COLOR"] = "never"
    return env


def candidate_source_files() -> list[Path]:
    files = [ROOT / "Cargo.toml"]
    for crate in CANDIDATE_CRATES:
        files.append(crate / "Cargo.toml")
        build_script = crate / "build.rs"
        if build_script.is_file():
            files.append(build_script)
        files.extend(sorted((crate / "src").rglob("*.rs")))
    return sorted(files)


def candidate_source_sha256() -> str:
    digest = hashlib.sha256()
    for path in candidate_source_files():
        digest.update(str(path.relative_to(ROOT)).encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def candidate_version() -> str:
    manifest = tomllib.loads(
        (ROOT / "crates/pdfplumber/Cargo.toml").read_text(encoding="utf-8")
    )
    return str(manifest["package"]["version"])


def require_clean_candidate_source() -> None:
    relative_paths = [str(path.relative_to(ROOT)) for path in CANDIDATE_CRATES]
    completed = command(
        ["git", "status", "--short", "--", *relative_paths], cwd=ROOT
    )
    if completed.stdout.strip():
        raise MeasurementError(
            "candidate crate source has uncommitted changes:\n" + completed.stdout
        )


def add_documented_dependency(
    manifest: Path, installation: str, candidate: Path
) -> None:
    current = manifest.read_text(encoding="utf-8")
    marker = "[dependencies]\n"
    if current.count(marker) != 1:
        raise MeasurementError(
            "cargo new manifest does not contain one empty [dependencies] table"
        )
    patched = current.replace(marker, f"{installation}\n")
    patched += (
        "\n[patch.crates-io]\n"
        f'pdfplumber = {{ path = "{candidate.as_posix()}" }}\n'
    )
    manifest.write_text(patched, encoding="utf-8")


def resolved_candidate_version(lock_path: Path) -> str:
    lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    matches = [
        package
        for package in lock.get("package", [])
        if package.get("name") == "pdfplumber"
        and "source" not in package
    ]
    if len(matches) != 1:
        raise MeasurementError(
            f"expected one path-patched pdfplumber package in Cargo.lock, observed {matches}"
        )
    return str(matches[0]["version"])


def rounded(seconds: float) -> float:
    return round(seconds, 3)


def measure(expected_version: str) -> dict[str, Any]:
    installation, snippet = measurement_inputs()
    require_clean_candidate_source()
    observed_candidate_version = candidate_version()
    if observed_candidate_version != expected_version:
        raise MeasurementError(
            f"expected candidate {expected_version}, observed {observed_candidate_version}"
        )
    source_sha256 = candidate_source_sha256()
    environment = {
        "os": f"{platform.system()} {platform.release()}",
        "architecture": platform.machine(),
        "rustc": version_output(["rustc", "-V"]),
        "cargo": version_output(["cargo", "-V"]),
    }

    phases: dict[str, float] = {}
    with tempfile.TemporaryDirectory(prefix="pdfplumber-rust-ttfv-") as temp:
        temp_root = Path(temp)
        cargo_home = temp_root / "cargo-home"
        cargo_home.mkdir()
        if any(cargo_home.iterdir()):
            raise MeasurementError("temporary CARGO_HOME was not empty at clock start")
        env = clean_cargo_environment(cargo_home)
        consumer = temp_root / "consumer"

        total_started = time.perf_counter()

        phase_started = time.perf_counter()
        command(
            [
                "cargo",
                "new",
                "--quiet",
                "--bin",
                "--vcs",
                "none",
                "--name",
                "pdfplumber-ttfv",
                "consumer",
            ],
            cwd=temp_root,
            env=env,
        )
        phases["project_creation"] = time.perf_counter() - phase_started
        if (consumer / "Cargo.lock").exists() or (consumer / "target").exists():
            raise MeasurementError("new project unexpectedly contained build state")

        phase_started = time.perf_counter()
        add_documented_dependency(
            consumer / "Cargo.toml", installation, ROOT / "crates/pdfplumber"
        )
        phases["dependency_declaration"] = time.perf_counter() - phase_started

        phase_started = time.perf_counter()
        (consumer / "src/main.rs").write_text(snippet + "\n", encoding="utf-8")
        shutil.copyfile(FIXTURE, consumer / "document.pdf")
        phases["code_copy"] = time.perf_counter() - phase_started

        phase_started = time.perf_counter()
        completed = command(["cargo", "run", "--quiet"], cwd=consumer, env=env)
        phases["resolve_build_execute"] = time.perf_counter() - phase_started

        phase_started = time.perf_counter()
        if quick_starts.PRIMARY_RUST_OUTPUT_MARKER not in completed.stdout:
            raise MeasurementError(
                "primary Rust quick start did not produce the expected extracted text"
            )
        phases["interpretation"] = time.perf_counter() - phase_started
        total_seconds = time.perf_counter() - total_started

        lock_path = consumer / "Cargo.lock"
        if not lock_path.is_file():
            raise MeasurementError("cargo run did not create Cargo.lock")
        resolved_version = resolved_candidate_version(lock_path)
        if resolved_version != expected_version:
            raise MeasurementError(
                f"expected pdfplumber {expected_version}, resolved {resolved_version}"
            )

        result = {
            "schema_version": SCHEMA_VERSION,
            "measured_at_utc": datetime.now(timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "result": "pass" if total_seconds <= TTFV_LIMIT_SECONDS else "fail",
            "threshold_seconds": TTFV_LIMIT_SECONDS,
            "total_seconds": rounded(total_seconds),
            "coverage": {component: True for component in COVERAGE},
            "phases": {name: rounded(phases[name]) for name in PHASES},
            "source": {
                "kind": "workspace candidate",
                "crate": "pdfplumber",
                "requirement": installation,
                "resolved_version": resolved_version,
                "tree_sha256": source_sha256,
            },
            "isolation": {
                "project": "new cargo project",
                "cargo_home": "empty temporary directory",
                "target": "new project-local directory",
                "cargo_lock_at_start": "absent",
                "candidate_substitution": "path patch outside the documented user steps",
            },
            "inputs": {
                "installation_sha256": sha256_text(installation),
                "quick_start_sha256": sha256_text(snippet),
                "fixture": str(FIXTURE.relative_to(ROOT)),
                "fixture_sha256": sha256_file(FIXTURE),
                "cargo_lock_sha256": sha256_file(lock_path),
            },
            "environment": environment,
            "commands": [CARGO_NEW_COMMAND, CARGO_RUN_COMMAND],
            "observation": {
                "kind": "automated clean-state lower bound",
                "network": "live crates.io access for transitive dependencies; latency is uncontrolled",
                "toolchain": "preinstalled rolling stable Rust prerequisite",
            },
            "registry_trial": {
                "version": "0.3.0",
                "result": "compile failure",
                "reason": "Pdf::open_path is absent from the published release",
                "disposition": "not counted as passing TTFV; DIST-001 and DIST-007 remain open",
            },
        }
    return result


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MeasurementError(message)


def require_sha256(value: object, name: str) -> None:
    require(
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value),
        f"{name} is not a lowercase SHA-256 digest",
    )


def check_result(path: Path) -> dict[str, Any]:
    try:
        result = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise MeasurementError(f"cannot read TTFV result {path}: {error}") from error

    installation, snippet = measurement_inputs()
    require(result.get("schema_version") == SCHEMA_VERSION, "schema version drift")
    require(result.get("result") == "pass", "recorded result is not pass")
    require(
        result.get("threshold_seconds") == TTFV_LIMIT_SECONDS,
        "five-minute threshold drift",
    )
    total = result.get("total_seconds")
    require(isinstance(total, (int, float)), "total_seconds is not numeric")
    require(0 <= total <= TTFV_LIMIT_SECONDS, "recorded TTFV exceeds five minutes")
    require(
        result.get("coverage") == {component: True for component in COVERAGE},
        "TTFV component coverage drift",
    )
    phases = result.get("phases")
    require(isinstance(phases, dict), "phases is not an object")
    require(set(phases) == set(PHASES), "phase inventory drift")
    require(
        all(isinstance(value, (int, float)) and value >= 0 for value in phases.values()),
        "phase duration is invalid",
    )

    source = result.get("source", {})
    require(
        source.get("kind") == "workspace candidate",
        "measurement is not current-source candidate evidence",
    )
    require(source.get("crate") == "pdfplumber", "measured crate drift")
    require(source.get("requirement") == installation, "installation snippet drift")
    require(bool(source.get("resolved_version")), "resolved version is absent")
    require(
        source.get("tree_sha256") == candidate_source_sha256(),
        "candidate source digest drift",
    )

    isolation = result.get("isolation", {})
    require(isolation.get("project") == "new cargo project", "project is not new")
    require(
        isolation.get("cargo_home") == "empty temporary directory",
        "Cargo home is not empty",
    )
    require(
        isolation.get("target") == "new project-local directory",
        "target directory is not project-local and new",
    )
    require(isolation.get("cargo_lock_at_start") == "absent", "lock was preseeded")
    require(
        isolation.get("candidate_substitution")
        == "path patch outside the documented user steps",
        "candidate substitution boundary drift",
    )

    inputs = result.get("inputs", {})
    require(
        inputs.get("installation_sha256") == sha256_text(installation),
        "installation digest drift",
    )
    require(
        inputs.get("quick_start_sha256") == sha256_text(snippet),
        "quick-start digest drift",
    )
    require(inputs.get("fixture") == str(FIXTURE.relative_to(ROOT)), "fixture drift")
    require(inputs.get("fixture_sha256") == sha256_file(FIXTURE), "fixture digest drift")
    require_sha256(inputs.get("cargo_lock_sha256"), "Cargo.lock digest")

    environment = result.get("environment", {})
    for key in ("os", "architecture", "rustc", "cargo"):
        require(bool(environment.get(key)), f"environment field {key} is absent")
    require(
        result.get("commands") == [CARGO_NEW_COMMAND, CARGO_RUN_COMMAND],
        "command inventory drift",
    )
    registry_trial = result.get("registry_trial", {})
    require(registry_trial.get("version") == "0.3.0", "registry trial version drift")
    require(
        registry_trial.get("result") == "compile failure",
        "registry failure is no longer explicit",
    )
    require(
        "Pdf::open_path" in registry_trial.get("reason", ""),
        "registry compile failure reason drift",
    )
    require(
        "DIST-001" in registry_trial.get("disposition", "")
        and "DIST-007" in registry_trial.get("disposition", ""),
        "registry follow-up tasks drift",
    )
    rendered = json.dumps(result, sort_keys=True)
    require(str(ROOT) not in rendered, "result contains a repository-local path")
    require(str(Path.home()) not in rendered, "result contains a home-directory path")
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--measure", action="store_true", help="run the clean measurement")
    mode.add_argument("--check", type=Path, metavar="RESULT", help="validate a result")
    parser.add_argument(
        "--expected-version",
        default=DEFAULT_CANDIDATE_VERSION,
        help="exact current-source version expected in the generated Cargo.lock",
    )
    parser.add_argument("--output", type=Path, help="write the measured JSON here")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.check is not None:
        result = check_result(args.check)
        print(
            "Rust TTFV result verified: "
            f"{result['total_seconds']:.3f}s <= {result['threshold_seconds']}s"
        )
        return 0

    result = measure(args.expected_version)
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        print(rendered, end="")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote {args.output}")
    if result["result"] != "pass":
        raise MeasurementError(
            f"Rust TTFV {result['total_seconds']:.3f}s exceeds "
            f"{result['threshold_seconds']}s"
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except MeasurementError as error:
        print(f"Rust TTFV measurement failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
