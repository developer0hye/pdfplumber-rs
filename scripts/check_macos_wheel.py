#!/usr/bin/env python3
"""Verify one macOS wheel's native artifact and installed behavior."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Callable, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

import tomllib


class MacosWheelError(ValueError):
    """Raised when a macOS wheel violates the release policy."""


Runner = Callable[..., subprocess.CompletedProcess[str]]


INSTALLED_PROBE = r"""
import hashlib
import importlib.metadata
import json
import platform
import sys
from pathlib import Path

import pdfplumber
from pdfplumber import _native

fixture = Path(sys.argv[1]).resolve()
expected_path = Path(sys.argv[2]).resolve()
expected_records = [
    json.loads(line)
    for line in expected_path.read_text(encoding="utf-8").splitlines()
    if line.strip()
]
if len(expected_records) != 1 or expected_records[0].get("page") != 1:
    raise RuntimeError("expected-output policy must contain exactly page 1")

with pdfplumber.open(fixture) as document:
    texts = [page.extract_text() for page in document.pages]
if texts != [expected_records[0].get("text")]:
    raise RuntimeError("installed wheel extraction does not match exact expected text")

prefix = Path(sys.prefix).resolve()
package_path = Path(pdfplumber.__file__).resolve()
native_path = Path(_native.__file__).resolve()
for label, path in (("package", package_path), ("native module", native_path)):
    if not path.is_relative_to(prefix):
        raise RuntimeError(f"installed {label} is outside the isolated environment: {path}")

print(
    json.dumps(
        {
            "distribution_version": importlib.metadata.version("pdfplumber-rs"),
            "python_version": platform.python_version(),
            "machine": platform.machine(),
            "package_file": package_path.relative_to(prefix).as_posix(),
            "native_module_file": native_path.relative_to(prefix).as_posix(),
            "page_count": len(texts),
            "text_sha256": hashlib.sha256(texts[0].encode("utf-8")).hexdigest(),
            "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
            "expected_sha256": hashlib.sha256(expected_path.read_bytes()).hexdigest(),
        },
        sort_keys=True,
    )
)
"""


def parse_args(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Verify one macOS wheel's filename, Mach-O metadata, native host, "
            "and installed extraction."
        )
    )
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--wheel-dir", type=Path, required=True)
    parser.add_argument("--installed-python", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(arguments)


def read_toml(path: Path) -> dict[str, Any]:
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as error:
        raise MacosWheelError(f"cannot read policy {path}: {error}") from error
    try:
        return tomllib.loads(content)
    except tomllib.TOMLDecodeError as error:
        raise MacosWheelError(f"invalid policy TOML {path}: {error}") from error


def require_string(mapping: dict[str, Any], field: str, context: str) -> str:
    value = mapping.get(field)
    if not isinstance(value, str) or not value:
        raise MacosWheelError(f"{context} requires a non-empty {field}")
    return value


def load_policy(path: Path) -> dict[str, dict[str, str]]:
    policy = read_toml(path)
    if policy.get("schema_version") != 1:
        raise MacosWheelError("macOS wheel policy requires schema_version = 1")

    raw_targets = policy.get("targets")
    if not isinstance(raw_targets, list) or not raw_targets:
        raise MacosWheelError("macOS wheel policy requires at least one target")

    required_fields = (
        "target",
        "runner",
        "runner_arch",
        "machine",
        "python_tag",
        "abi_tag",
        "platform_tag",
        "deployment_target",
        "macho_arch",
    )
    targets: dict[str, dict[str, str]] = {}
    for index, raw_target in enumerate(raw_targets):
        context = f"target policy #{index + 1}"
        if not isinstance(raw_target, dict):
            raise MacosWheelError(f"{context} must be a table")
        target = {
            field: require_string(raw_target, field, context)
            for field in required_fields
        }
        target_name = target["target"]
        if target_name in targets:
            raise MacosWheelError(f"macOS wheel policy repeats target {target_name}")
        targets[target_name] = target
    return targets


def validate_host(
    target_policy: dict[str, str],
    system: str,
    machine: str,
    runner_arch: str,
) -> None:
    if system != "Darwin":
        raise MacosWheelError(f"wheel verification requires macOS, found {system!r}")
    expected_machine = target_policy["machine"]
    if machine != expected_machine:
        raise MacosWheelError(
            f"native runner machine {machine!r} does not equal {expected_machine!r}"
        )
    expected_runner_arch = target_policy["runner_arch"]
    if runner_arch != expected_runner_arch:
        raise MacosWheelError(
            f"RUNNER_ARCH {runner_arch!r} does not equal {expected_runner_arch!r}"
        )


def validate_wheel_name(wheel_name: str, target_policy: dict[str, str]) -> None:
    expected_suffix = (
        f"-{target_policy['python_tag']}-{target_policy['abi_tag']}-"
        f"{target_policy['platform_tag']}.whl"
    )
    if not wheel_name.endswith(expected_suffix):
        raise MacosWheelError(
            f"wheel filename does not carry exact policy tag {expected_suffix}: "
            f"{wheel_name}"
        )


def parse_deployment_target(load_commands: str) -> str:
    commands: list[tuple[str, str]] = []
    blocks = re.split(r"(?m)(?=^Load command \d+\s*$)", load_commands)
    for block in blocks:
        if re.search(r"(?m)^\s*cmd LC_VERSION_MIN_MACOSX\s*$", block):
            match = re.search(r"(?m)^\s*version\s+([0-9]+(?:\.[0-9]+)+)\s*$", block)
            if match is None:
                raise MacosWheelError("LC_VERSION_MIN_MACOSX omits version")
            commands.append(("LC_VERSION_MIN_MACOSX", match.group(1)))
        if re.search(r"(?m)^\s*cmd LC_BUILD_VERSION\s*$", block):
            platform_match = re.search(r"(?m)^\s*platform\s+([0-9]+)\s*$", block)
            if platform_match is None or platform_match.group(1) != "1":
                raise MacosWheelError("LC_BUILD_VERSION is not for the macOS platform")
            match = re.search(r"(?m)^\s*minos\s+([0-9]+(?:\.[0-9]+)+)\s*$", block)
            if match is None:
                raise MacosWheelError("LC_BUILD_VERSION omits minos")
            commands.append(("LC_BUILD_VERSION", match.group(1)))
    if len(commands) != 1:
        raise MacosWheelError(
            "native module must contain exactly one macOS deployment load command"
        )
    return commands[0][1]


def validate_macho(inspection: dict[str, Any], target_policy: dict[str, str]) -> None:
    architectures = inspection.get("architectures")
    expected_architecture = target_policy["macho_arch"]
    if architectures != [expected_architecture]:
        raise MacosWheelError(
            f"Mach-O architectures {architectures!r} do not equal "
            f"[{expected_architecture!r}]"
        )
    deployment_target = inspection.get("deployment_target")
    expected_target = target_policy["deployment_target"]
    if deployment_target != expected_target:
        raise MacosWheelError(
            f"Mach-O deployment target {deployment_target!r} does not equal "
            f"{expected_target!r}"
        )


def run_tool(command: tuple[str, ...], runner: Runner) -> str:
    try:
        result = runner(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise MacosWheelError(f"cannot execute {command[0]}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise MacosWheelError(f"{command[0]} exited with {result.returncode}: {detail}")
    return result.stdout


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as input_file:
            for chunk in iter(lambda: input_file.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise MacosWheelError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def inspect_native_module(
    native_path: Path,
    target_policy: dict[str, str],
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    architectures = run_tool(("lipo", "-archs", str(native_path)), runner).split()
    load_commands = run_tool(("otool", "-l", str(native_path)), runner)
    inspection = {
        "architectures": architectures,
        "deployment_target": parse_deployment_target(load_commands),
    }
    validate_macho(inspection, target_policy)
    return inspection


def inspect_wheel(
    wheel_path: Path,
    target_policy: dict[str, str],
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    if wheel_path.is_symlink() or not wheel_path.is_file():
        raise MacosWheelError(f"wheel must be a real file: {wheel_path}")
    validate_wheel_name(wheel_path.name, target_policy)
    try:
        with zipfile.ZipFile(wheel_path) as archive:
            native_names = [
                name
                for name in archive.namelist()
                if name.startswith("pdfplumber/_native") and name.endswith("-darwin.so")
            ]
            if len(native_names) != 1:
                raise MacosWheelError(
                    "wheel must contain exactly one private Darwin native module"
                )
            native_name = native_names[0]
            archive_path = PurePosixPath(native_name)
            if archive_path.is_absolute() or ".." in archive_path.parts:
                raise MacosWheelError(
                    f"unsafe native module path in wheel: {native_name}"
                )
            native_bytes = archive.read(native_name)
    except (OSError, zipfile.BadZipFile, KeyError) as error:
        raise MacosWheelError(f"cannot inspect wheel {wheel_path}: {error}") from error

    with tempfile.TemporaryDirectory() as temporary_directory:
        native_path = Path(temporary_directory) / "native-module.so"
        native_path.write_bytes(native_bytes)
        inspection = inspect_native_module(native_path, target_policy, runner=runner)
    return {
        "native_module": native_name,
        "native_module_sha256": sha256_bytes(native_bytes),
        **inspection,
    }


def validate_installed_probe(
    probe: dict[str, Any], target_policy: dict[str, str]
) -> None:
    if probe.get("machine") != target_policy["machine"]:
        raise MacosWheelError("installed probe ran on the wrong machine architecture")
    python_version = probe.get("python_version")
    if not isinstance(python_version, str) or not python_version.startswith("3.13."):
        raise MacosWheelError("installed probe did not run on CPython 3.13")
    if probe.get("page_count") != 1:
        raise MacosWheelError("installed probe did not extract exactly one page")
    for field in (
        "distribution_version",
        "package_file",
        "native_module_file",
        "text_sha256",
        "fixture_sha256",
        "expected_sha256",
    ):
        value = probe.get(field)
        if not isinstance(value, str) or not value:
            raise MacosWheelError(f"installed probe requires {field}")
    if "site-packages/pdfplumber/" not in probe["package_file"]:
        raise MacosWheelError("installed package was not imported from site-packages")
    if "site-packages/pdfplumber/_native" not in probe["native_module_file"]:
        raise MacosWheelError(
            "installed native module was not imported from site-packages"
        )


def run_installed_probe(
    installed_python: Path,
    fixture: Path,
    expected: Path,
    target_policy: dict[str, str],
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    for path, label in (
        (installed_python, "installed Python"),
        (fixture, "fixture"),
        (expected, "expected output"),
    ):
        if not path.is_file():
            raise MacosWheelError(f"{label} does not exist: {path}")
    output = run_tool(
        (
            str(installed_python),
            "-I",
            "-c",
            INSTALLED_PROBE,
            str(fixture),
            str(expected),
        ),
        runner,
    )
    try:
        probe = json.loads(output)
    except json.JSONDecodeError as error:
        raise MacosWheelError(
            f"installed probe returned invalid JSON: {error}"
        ) from error
    if not isinstance(probe, dict):
        raise MacosWheelError("installed probe JSON root must be an object")
    validate_installed_probe(probe, target_policy)
    if probe["fixture_sha256"] != sha256_file(fixture):
        raise MacosWheelError("installed probe fixture digest does not match the input")
    if probe["expected_sha256"] != sha256_file(expected):
        raise MacosWheelError(
            "installed probe expected-output digest does not match the input"
        )
    return probe


def build_evidence(
    wheel_path: Path,
    policy_path: Path,
    target_policy: dict[str, str],
    inspection: dict[str, Any],
    probe: dict[str, Any],
) -> dict[str, Any]:
    if policy_path.is_symlink() or not policy_path.is_file():
        raise MacosWheelError(f"policy must be a real file: {policy_path}")
    validate_wheel_name(wheel_path.name, target_policy)
    validate_macho(inspection, target_policy)
    validate_installed_probe(probe, target_policy)
    return {
        "schema_version": 1,
        "outcome": "compatible",
        "target": target_policy["target"],
        "wheel": wheel_path.name,
        "wheel_sha256": sha256_file(wheel_path),
        "policy": policy_path.name,
        "policy_sha256": sha256_file(policy_path),
        "native_runner": {
            "label": target_policy["runner"],
            "runner_arch": target_policy["runner_arch"],
            "machine": target_policy["machine"],
        },
        "macho": inspection,
        "installed_probe": probe,
    }


def write_json(path: Path, payload: dict[str, Any]) -> None:
    if path.is_symlink():
        raise MacosWheelError(f"refusing to replace symlink output: {path}")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError as error:
        raise MacosWheelError(f"cannot write evidence {path}: {error}") from error


def main(arguments: Sequence[str] | None = None) -> int:
    options = parse_args(arguments)
    try:
        targets = load_policy(options.policy)
        target_policy = targets.get(options.target)
        if target_policy is None:
            raise MacosWheelError(f"unknown macOS wheel target {options.target!r}")
        validate_host(
            target_policy,
            platform.system(),
            platform.machine(),
            os.environ.get("RUNNER_ARCH", ""),
        )
        if options.wheel_dir.is_symlink() or not options.wheel_dir.is_dir():
            raise MacosWheelError(
                f"wheel directory must be a real directory: {options.wheel_dir}"
            )
        wheels = sorted(options.wheel_dir.glob("*.whl"), key=lambda path: path.name)
        if len(wheels) != 1:
            raise MacosWheelError(
                f"expected exactly one wheel in {options.wheel_dir}, found {len(wheels)}"
            )
        wheel_path = wheels[0]
        inspection = inspect_wheel(wheel_path, target_policy)
        probe = run_installed_probe(
            options.installed_python,
            options.fixture,
            options.expected,
            target_policy,
        )
        evidence = build_evidence(
            wheel_path,
            options.policy,
            target_policy,
            inspection,
            probe,
        )
        write_json(options.output, evidence)
    except MacosWheelError as error:
        print(f"macOS wheel verification failed: {error}", file=sys.stderr)
        return 1

    print(
        f"target={options.target} wheel={wheel_path.name} "
        f"architecture={inspection['architectures'][0]} "
        f"deployment_target={inspection['deployment_target']} "
        f"installed_version={probe['distribution_version']} outcome=compatible",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
