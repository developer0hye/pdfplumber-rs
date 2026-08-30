#!/usr/bin/env python3
"""Install one exact release from a public registry and exercise a real PDF."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from build_cli_release import CliReleaseError, load_smoke_policy
    from check_wasm_package import (
        WasmPackageError,
        expected_text,
        run_browser_consumer,
        sha256_bytes,
        sha256_file,
        validate_runtime_result,
    )
    from check_wasm_package import (
        load_policy as load_wasm_policy,
    )
    from release_version import VERSION_PATTERN, load_release_identity
except ModuleNotFoundError:
    from scripts.build_cli_release import CliReleaseError, load_smoke_policy
    from scripts.check_wasm_package import (
        WasmPackageError,
        expected_text,
        run_browser_consumer,
        sha256_bytes,
        sha256_file,
        validate_runtime_result,
    )
    from scripts.check_wasm_package import (
        load_policy as load_wasm_policy,
    )
    from scripts.release_version import VERSION_PATTERN, load_release_identity


ROOT = Path(__file__).resolve().parents[1]
PUBLIC_FAMILIES = ("crates", "pypi", "npm")
CRATES_API_ROOT = "https://crates.io/api/v1/crates/pdfplumber-cli"
PYPI_INDEX = "https://pypi.org/simple"
PYPI_JSON_ROOT = "https://pypi.org/pypi/pdfplumber-rs"
NPM_REGISTRY = "https://registry.npmjs.org"
NPM_PACKAGE_ROOT = "https://registry.npmjs.org/pdfplumber-wasm"
DEFAULT_TIMEOUT_SECONDS = 600.0
DEFAULT_INITIAL_DELAY_SECONDS = 5.0
DEFAULT_MAXIMUM_DELAY_SECONDS = 30.0
DEFAULT_PROBE_TIMEOUT_SECONDS = 30.0
DEFAULT_INSTALL_TIMEOUT_SECONDS = 1_200.0
DEFAULT_WASM_POLICY = ROOT / "wasm-package-test-policy.toml"
DEFAULT_WASM_TOOLS = ROOT / "compat" / "wasm-package-tests"
PYTHON_SMOKE_PROGRAM = r"""
import hashlib
import importlib.metadata
import json
from pathlib import Path
import sys

import pdfplumber
from pdfplumber import _native

version, fixture_name, expected_name = sys.argv[1:]
fixture = Path(fixture_name).resolve()
expected = Path(expected_name).resolve()
expected_bytes = expected.read_bytes()
records = [json.loads(line) for line in expected_bytes.splitlines() if line]
if len(records) != 1 or records[0].get("page") != 1:
    raise RuntimeError("expected output must contain exactly page 1")
distribution_version = importlib.metadata.version("pdfplumber-rs")
if distribution_version != version or _native.__version__ != version:
    raise RuntimeError("installed Python distribution version drift")
environment = Path(sys.prefix).resolve()
for label, location in (
    ("package", pdfplumber.__file__),
    ("native module", _native.__file__),
):
    if location is None or not Path(location).resolve().is_relative_to(environment):
        raise RuntimeError(f"installed {label} is outside the isolated environment")
with pdfplumber.open(fixture) as document:
    texts = [page.extract_text() for page in document.pages]
if texts != [records[0]["text"]]:
    raise RuntimeError("installed Python package did not produce exact fixture text")
print(json.dumps({
    "distribution_version": distribution_version,
    "page_count": len(texts),
    "text_sha256": hashlib.sha256(texts[0].encode("utf-8")).hexdigest(),
    "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
    "expected_sha256": hashlib.sha256(expected_bytes).hexdigest(),
    "package_path": str(Path(pdfplumber.__file__).resolve()),
    "native_path": str(Path(_native.__file__).resolve()),
}, sort_keys=True))
""".strip()


class PublicRegistryError(RuntimeError):
    """A public artifact could not be resolved, installed, or exercised."""


@dataclass(frozen=True)
class RegistryResult:
    """Bounded registry-resolution evidence."""

    family: str
    version: str
    attempts: int
    elapsed_seconds: float


Probe = Callable[[str, str, float], bool]
Clock = Callable[[], float]
Sleeper = Callable[[float], None]
Emitter = Callable[[str], None]


def release_version_from_tag(release_tag: str, workspace_version: str) -> str:
    """Require a strict release tag for the exact workspace version."""

    if VERSION_PATTERN.fullmatch(workspace_version) is None:
        raise PublicRegistryError(
            f"workspace version must use strict X.Y.Z: {workspace_version!r}"
        )
    expected_tag = f"v{workspace_version}"
    if release_tag != expected_tag:
        raise PublicRegistryError(
            f"release tag {release_tag!r} does not match workspace {expected_tag!r}"
        )
    return workspace_version


def cargo_install_command(version: str, root: Path) -> tuple[str, ...]:
    """Build the exact crates.io-only Command-Line Interface install command."""

    return (
        "cargo",
        "install",
        "pdfplumber-cli",
        "--version",
        f"={version}",
        "--registry",
        "crates-io",
        "--locked",
        "--root",
        str(root),
        "--color",
        "never",
    )


def pypi_install_command(python: Path, version: str) -> tuple[str, ...]:
    """Build the exact wheel-only PyPI install command."""

    return (
        str(python),
        "-m",
        "pip",
        "install",
        "--disable-pip-version-check",
        "--no-cache-dir",
        "--only-binary=:all:",
        "--index-url",
        PYPI_INDEX,
        f"pdfplumber-rs=={version}",
    )


def npm_install_command(version: str) -> tuple[str, ...]:
    """Build the exact public-npm consumer install command."""

    return (
        "npm",
        "install",
        "--ignore-scripts",
        "--no-audit",
        "--no-fund",
        "--package-lock=false",
        "--save-exact",
        "--registry",
        NPM_REGISTRY,
        f"pdfplumber-wasm@{version}",
    )


def positive_seconds(value: str) -> float:
    """Parse a finite positive duration for argparse."""

    try:
        seconds = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not math.isfinite(seconds) or seconds <= 0:
        raise argparse.ArgumentTypeError("must be finite and greater than zero")
    return seconds


def validate_wait_policy(
    family: str,
    version: str,
    timeout_seconds: float,
    initial_delay_seconds: float,
    maximum_delay_seconds: float,
    probe_timeout_seconds: float,
) -> None:
    """Reject unknown registries and unbounded or ambiguous policy."""

    if family not in PUBLIC_FAMILIES:
        raise PublicRegistryError(f"unsupported public registry family: {family!r}")
    if VERSION_PATTERN.fullmatch(version) is None:
        raise PublicRegistryError(f"version must use strict X.Y.Z: {version!r}")
    for label, value in (
        ("timeout_seconds", timeout_seconds),
        ("initial_delay_seconds", initial_delay_seconds),
        ("maximum_delay_seconds", maximum_delay_seconds),
        ("probe_timeout_seconds", probe_timeout_seconds),
    ):
        if not math.isfinite(value) or value <= 0:
            raise PublicRegistryError(f"{label} must be finite and greater than zero")
    if maximum_delay_seconds < initial_delay_seconds:
        raise PublicRegistryError(
            "maximum_delay_seconds must be at least initial_delay_seconds"
        )


def run_command(
    command: Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run one checked command without inheriting a checkout-relative directory."""

    try:
        completed = subprocess.run(
            tuple(command),
            cwd=cwd,
            env=env,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PublicRegistryError(
            f"command could not complete: {command[0]}"
        ) from error
    if completed.returncode != 0:
        stdout = completed.stdout[-4_000:]
        stderr = completed.stderr[-4_000:]
        raise PublicRegistryError(
            f"command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{stdout}\nstderr:\n{stderr}"
        )
    return completed


def fetch_public_json(url: str, timeout_seconds: float) -> Any | None:
    """Fetch uncached public registry metadata without a package-client cache."""

    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "Cache-Control": "no-cache",
            "User-Agent": "pdfplumber-rs-public-registry-gate",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            return json.load(response)
    except (
        TimeoutError,
        OSError,
        UnicodeError,
        ValueError,
        urllib.error.URLError,
    ):
        return None


def probe_registry(family: str, version: str, timeout_seconds: float) -> bool:
    """Return whether one exact version is visible at its public endpoint."""

    if family == "crates":
        payload = fetch_public_json(f"{CRATES_API_ROOT}/{version}", timeout_seconds)
        published = payload.get("version") if isinstance(payload, dict) else None
        return (
            isinstance(published, dict)
            and published.get("crate") == "pdfplumber-cli"
            and published.get("num") == version
            and published.get("yanked") is False
        )
    if family == "npm":
        payload = fetch_public_json(f"{NPM_PACKAGE_ROOT}/{version}", timeout_seconds)
        distribution = payload.get("dist") if isinstance(payload, dict) else None
        return (
            isinstance(payload, dict)
            and payload.get("name") == "pdfplumber-wasm"
            and payload.get("version") == version
            and isinstance(distribution, dict)
            and isinstance(distribution.get("tarball"), str)
        )

    payload = fetch_public_json(f"{PYPI_JSON_ROOT}/{version}/json", timeout_seconds)
    info = payload.get("info") if isinstance(payload, dict) else None
    urls = payload.get("urls") if isinstance(payload, dict) else None
    return (
        isinstance(info, dict)
        and info.get("version") == version
        and isinstance(urls, list)
        and len(urls) > 0
    )


def wait_for_registry(
    family: str,
    version: str,
    *,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    initial_delay_seconds: float = DEFAULT_INITIAL_DELAY_SECONDS,
    maximum_delay_seconds: float = DEFAULT_MAXIMUM_DELAY_SECONDS,
    probe_timeout_seconds: float = DEFAULT_PROBE_TIMEOUT_SECONDS,
    probe: Probe = probe_registry,
    monotonic: Clock = time.monotonic,
    sleeper: Sleeper = time.sleep,
    emit: Emitter = print,
) -> RegistryResult:
    """Poll one public registry with capped exponential backoff."""

    validate_wait_policy(
        family,
        version,
        timeout_seconds,
        initial_delay_seconds,
        maximum_delay_seconds,
        probe_timeout_seconds,
    )
    start_seconds = monotonic()
    deadline_seconds = start_seconds + timeout_seconds
    delay_seconds = initial_delay_seconds
    attempts = 0
    while True:
        now_seconds = monotonic()
        if attempts > 0 and now_seconds >= deadline_seconds:
            elapsed_seconds = now_seconds - start_seconds
            raise PublicRegistryError(
                f"timed out waiting for {family} version {version} after "
                f"{attempts} attempts and {elapsed_seconds:.3f}s"
            )
        attempts += 1
        current_probe_timeout = min(
            probe_timeout_seconds, deadline_seconds - now_seconds
        )
        emit(
            "INFO public_registry_probe "
            f"family={family} version={version} attempt={attempts}"
        )
        if probe(family, version, current_probe_timeout):
            elapsed_seconds = monotonic() - start_seconds
            return RegistryResult(family, version, attempts, elapsed_seconds)
        now_seconds = monotonic()
        remaining_seconds = deadline_seconds - now_seconds
        if remaining_seconds <= 0:
            continue
        current_delay = min(delay_seconds, remaining_seconds)
        emit(
            "WARN public_registry_probe "
            f"family={family} version={version} attempt={attempts} "
            f"next_delay_seconds={current_delay:.3f}"
        )
        sleeper(current_delay)
        delay_seconds = min(delay_seconds * 2, maximum_delay_seconds)


def parse_json_line(output: str, label: str) -> dict[str, Any]:
    """Read the final non-empty output line as an object."""

    lines = [line for line in output.splitlines() if line.strip()]
    if not lines:
        raise PublicRegistryError(f"{label} produced no JSON result")
    try:
        payload = json.loads(lines[-1])
    except json.JSONDecodeError as error:
        raise PublicRegistryError(f"{label} result was not JSON") from error
    if not isinstance(payload, dict):
        raise PublicRegistryError(f"{label} JSON result must be an object")
    return payload


def run_crates_smoke(
    version: str, root: Path, install_timeout_seconds: float
) -> dict[str, Any]:
    """Install the exact CLI from crates.io and assert byte-exact output."""

    policy = load_smoke_policy(ROOT / "cli-release-smoke.toml")
    install_root = root / "cargo-install"
    install_root.mkdir()
    cargo_home = root / "cargo-home"
    cargo_home.mkdir()
    cargo_environment = os.environ.copy()
    cargo_environment.update(
        {
            "CARGO_HOME": str(cargo_home),
            "CARGO_TARGET_DIR": str(root / "cargo-target"),
        }
    )
    run_command(
        cargo_install_command(version, install_root),
        cwd=root,
        timeout_seconds=install_timeout_seconds,
        env=cargo_environment,
    )
    executable = install_root / "bin" / "pdfplumber"
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise PublicRegistryError(f"cargo install did not create {executable}")
    arguments = tuple(
        str(policy.fixture_path) if value == "{fixture}" else value
        for value in policy.args
    )
    completed = run_command(
        (str(executable), *arguments),
        cwd=root,
        timeout_seconds=float(policy.timeout_seconds),
    )
    expected_stdout = policy.expected_stdout_path.read_text(encoding="utf-8")
    if completed.stderr:
        raise PublicRegistryError("installed crates.io CLI wrote to standard error")
    if completed.stdout != expected_stdout:
        raise PublicRegistryError(
            "installed crates.io CLI output did not match the exact fixture policy"
        )
    return {
        "package": "pdfplumber-cli",
        "executable": str(executable),
        "fixture_sha256": policy.fixture_sha256,
        "expected_sha256": policy.expected_stdout_sha256,
        "stdout_sha256": hashlib.sha256(completed.stdout.encode()).hexdigest(),
    }


def run_pypi_smoke(
    version: str, root: Path, install_timeout_seconds: float
) -> dict[str, Any]:
    """Install the exact public wheel in an isolated CPython 3.13 environment."""

    if platform.python_implementation() != "CPython" or sys.version_info[:2] != (
        3,
        13,
    ):
        raise PublicRegistryError("PyPI smoke requires CPython 3.13")
    environment = root / "venv"
    run_command(
        (sys.executable, "-m", "venv", str(environment)),
        cwd=root,
        timeout_seconds=install_timeout_seconds,
    )
    python = environment / "bin" / "python"
    pip_environment = {
        key: value for key, value in os.environ.items() if not key.startswith("PIP_")
    }
    pip_environment.update(
        {"PIP_CONFIG_FILE": os.devnull, "PIP_DISABLE_PIP_VERSION_CHECK": "1"}
    )
    run_command(
        pypi_install_command(python, version),
        cwd=root,
        timeout_seconds=install_timeout_seconds,
        env=pip_environment,
    )
    policy = load_smoke_policy(ROOT / "cli-release-smoke.toml")
    completed = run_command(
        (
            str(python),
            "-I",
            "-c",
            PYTHON_SMOKE_PROGRAM,
            version,
            str(policy.fixture_path),
            str(policy.expected_stdout_path),
        ),
        cwd=root,
        timeout_seconds=float(policy.timeout_seconds),
    )
    result = parse_json_line(completed.stdout, "installed PyPI package")
    expected = {
        "distribution_version": version,
        "page_count": 1,
        "fixture_sha256": policy.fixture_sha256,
        "expected_sha256": policy.expected_stdout_sha256,
    }
    for field, value in expected.items():
        if result.get(field) != value:
            raise PublicRegistryError(
                f"installed PyPI result {field} drift: {result.get(field)!r} != {value!r}"
            )
    return result


def run_npm_browser_smoke(
    version: str,
    root: Path,
    tools: Path,
    install_timeout_seconds: float,
) -> dict[str, Any]:
    """Install the exact npm package and exercise its bundler build in Chromium."""

    policy = load_wasm_policy(DEFAULT_WASM_POLICY)
    fixture = (ROOT / policy["fixture"]).resolve()
    expected = (ROOT / policy["expected"]).resolve()
    consumer = root / "browser-consumer"
    consumer.mkdir()
    (consumer / "package.json").write_text(
        json.dumps(
            {
                "name": "pdfplumber-wasm-public-registry-smoke",
                "version": "0.0.0",
                "private": True,
                "type": "module",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    npm_cache = root / "npm-cache"
    npm_cache.mkdir()
    npm_environment = os.environ.copy()
    npm_environment["npm_config_cache"] = str(npm_cache)
    run_command(
        npm_install_command(version),
        cwd=consumer,
        timeout_seconds=install_timeout_seconds,
        env=npm_environment,
    )
    package = consumer / "node_modules" / "pdfplumber-wasm"
    manifest_path = package / "package.json"
    if not manifest_path.is_file():
        raise PublicRegistryError("npm install omitted pdfplumber-wasm/package.json")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("name") != "pdfplumber-wasm" or manifest.get("version") != version:
        raise PublicRegistryError("installed npm package identity drift")
    wasm_files = sorted(package.glob("*.wasm"))
    if len(wasm_files) != 1:
        raise PublicRegistryError(
            f"installed npm package must contain exactly one WebAssembly binary: {wasm_files}"
        )
    result, browser_version = run_browser_consumer(
        consumer, tools.resolve(), fixture, expected
    )
    text_sha256 = sha256_bytes(expected_text(expected).encode("utf-8"))
    validate_runtime_result(
        result,
        "browser",
        text_sha256,
        sha256_file(fixture),
        sha256_file(expected),
    )
    return {
        "package": "pdfplumber-wasm",
        "package_version": manifest["version"],
        "package_path": str(package.resolve()),
        "wasm_sha256": sha256_file(wasm_files[0]),
        "browser": browser_version,
        "runtime": result,
    }


def source_commit() -> str:
    """Return the exact clean checkout commit recorded with the smoke result."""

    completed = run_command(
        ("git", "rev-parse", "HEAD"), cwd=ROOT, timeout_seconds=30.0
    )
    commit = completed.stdout.strip()
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise PublicRegistryError(f"invalid source commit: {commit!r}")
    status = run_command(
        ("git", "status", "--porcelain=v1", "--untracked-files=all"),
        cwd=ROOT,
        timeout_seconds=30.0,
    ).stdout
    if status.strip():
        raise PublicRegistryError(f"source checkout is dirty:\n{status.rstrip()}")
    return commit


def write_report(path: Path, report: dict[str, Any]) -> None:
    """Retain the success or failure state for the workflow run."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("family", choices=PUBLIC_FAMILIES)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--timeout-seconds",
        type=positive_seconds,
        default=DEFAULT_TIMEOUT_SECONDS,
    )
    parser.add_argument(
        "--initial-delay-seconds",
        type=positive_seconds,
        default=DEFAULT_INITIAL_DELAY_SECONDS,
    )
    parser.add_argument(
        "--maximum-delay-seconds",
        type=positive_seconds,
        default=DEFAULT_MAXIMUM_DELAY_SECONDS,
    )
    parser.add_argument(
        "--probe-timeout-seconds",
        type=positive_seconds,
        default=DEFAULT_PROBE_TIMEOUT_SECONDS,
    )
    parser.add_argument(
        "--install-timeout-seconds",
        type=positive_seconds,
        default=DEFAULT_INSTALL_TIMEOUT_SECONDS,
    )
    parser.add_argument("--wasm-tools", type=Path, default=DEFAULT_WASM_TOOLS)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    version = "unknown"
    report: dict[str, Any] = {
        "schema_version": 1,
        "family": args.family,
        "release_tag": args.release_tag,
        "observed_at": datetime.now(timezone.utc).isoformat(),
        "outcome": "failed",
    }
    try:
        release = load_release_identity(ROOT)
        version = release_version_from_tag(args.release_tag, release.version)
        report["version"] = version
        report["source_commit"] = source_commit()
        resolution = wait_for_registry(
            args.family,
            version,
            timeout_seconds=args.timeout_seconds,
            initial_delay_seconds=args.initial_delay_seconds,
            maximum_delay_seconds=args.maximum_delay_seconds,
            probe_timeout_seconds=args.probe_timeout_seconds,
        )
        with tempfile.TemporaryDirectory(
            prefix=f"pdfplumber-{args.family}-public-"
        ) as temporary:
            temporary_root = Path(temporary).resolve()
            if args.family == "crates":
                smoke = run_crates_smoke(
                    version, temporary_root, args.install_timeout_seconds
                )
            elif args.family == "pypi":
                smoke = run_pypi_smoke(
                    version, temporary_root, args.install_timeout_seconds
                )
            else:
                smoke = run_npm_browser_smoke(
                    version,
                    temporary_root,
                    args.wasm_tools,
                    args.install_timeout_seconds,
                )
        report.update(
            {
                "outcome": "passed",
                "resolution": {
                    "attempts": resolution.attempts,
                    "elapsed_seconds": resolution.elapsed_seconds,
                },
                "smoke": smoke,
            }
        )
    except (
        CliReleaseError,
        OSError,
        PublicRegistryError,
        ValueError,
        WasmPackageError,
    ) as error:
        report.setdefault("version", version)
        report["error"] = str(error)
        write_report(args.output, report)
        print(
            f"public registry release check failed: family={args.family} "
            f"version={version} reason={error}",
            file=sys.stderr,
        )
        return 1
    write_report(args.output, report)
    print(
        f"public registry release check passed: family={args.family} "
        f"version={version} attempts={resolution.attempts}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
