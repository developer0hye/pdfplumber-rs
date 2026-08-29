#!/usr/bin/env python3
"""Verify Linux wheel tags and auditwheel shared-library results."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

import tomllib


class LinuxWheelError(ValueError):
    """Raised when a Linux wheel violates the release policy."""


def parse_args(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify one built Linux wheel with the pinned auditwheel policy."
    )
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--wheel-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(arguments)


def read_toml(path: Path) -> dict[str, Any]:
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as error:
        raise LinuxWheelError(f"cannot read policy {path}: {error}") from error
    try:
        return tomllib.loads(content)
    except tomllib.TOMLDecodeError as error:
        raise LinuxWheelError(f"invalid policy TOML {path}: {error}") from error


def require_string(mapping: dict[str, Any], field: str, context: str) -> str:
    value = mapping.get(field)
    if not isinstance(value, str) or not value:
        raise LinuxWheelError(f"{context} requires a non-empty {field}")
    return value


def load_policy(path: Path) -> dict[str, dict[str, Any]]:
    policy = read_toml(path)
    if policy.get("schema_version") != 1:
        raise LinuxWheelError("Linux wheel policy requires schema_version = 1")

    auditor = policy.get("auditor")
    if not isinstance(auditor, dict):
        raise LinuxWheelError("Linux wheel policy requires an auditor table")
    for field in (
        "auditwheel_version",
        "packaging_version",
        "pyelftools_version",
    ):
        require_string(auditor, field, "auditor policy")

    raw_targets = policy.get("targets")
    if not isinstance(raw_targets, list) or not raw_targets:
        raise LinuxWheelError("Linux wheel policy requires at least one target")

    targets: dict[str, dict[str, Any]] = {}
    required_fields = (
        "target",
        "python_tag",
        "abi_tag",
        "manylinux",
        "auditwheel_tag",
        "filename_platform_tag",
    )
    for index, raw_target in enumerate(raw_targets):
        context = f"target policy #{index + 1}"
        if not isinstance(raw_target, dict):
            raise LinuxWheelError(f"{context} must be a table")
        target = {field: require_string(raw_target, field, context) for field in required_fields}
        target_name = target["target"]
        if target_name in targets:
            raise LinuxWheelError(f"Linux wheel policy repeats target {target_name}")
        targets[target_name] = {**target, "auditor": dict(auditor)}
    return targets


def validate_auditwheel_report(
    report: dict[str, Any],
    target_policy: dict[str, Any],
    wheel_name: str,
) -> None:
    expected_suffix = (
        f"-{target_policy['python_tag']}-{target_policy['abi_tag']}-"
        f"{target_policy['filename_platform_tag']}.whl"
    )
    if not wheel_name.endswith(expected_suffix):
        raise LinuxWheelError(
            f"wheel filename does not carry exact policy tag {expected_suffix}: "
            f"{wheel_name}"
        )
    if report.get("version") != 1:
        raise LinuxWheelError("auditwheel report requires schema version 1")
    if report.get("wheel") != wheel_name:
        raise LinuxWheelError(
            f"auditwheel report names {report.get('wheel')!r}, expected {wheel_name!r}"
        )
    if report.get("pure") is not False:
        raise LinuxWheelError("auditwheel report must describe a native wheel")

    expected_tag = target_policy["auditwheel_tag"]
    if report.get("overall_tag") != expected_tag:
        raise LinuxWheelError(
            f"auditwheel overall tag {report.get('overall_tag')!r} "
            f"does not equal {expected_tag!r}"
        )
    if report.get("sym_tag") != expected_tag:
        raise LinuxWheelError(
            f"auditwheel symbol tag {report.get('sym_tag')!r} "
            f"does not equal {expected_tag!r}"
        )
    if report.get("unsupported_isa") is not False:
        raise LinuxWheelError("wheel requires an unsupported instruction-set extension")

    external_libraries = report.get("external_libs")
    if not isinstance(external_libraries, dict):
        raise LinuxWheelError("auditwheel report omits the external shared libraries map")
    if external_libraries:
        raise LinuxWheelError(
            "wheel requires external shared libraries outside the manylinux policy: "
            f"{sorted(external_libraries)}"
        )
    for field in ("versioned_symbols", "policy_upgrades"):
        if not isinstance(report.get(field), dict):
            raise LinuxWheelError(f"auditwheel report requires a {field} map")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as input_file:
            for chunk in iter(lambda: input_file.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise LinuxWheelError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def build_evidence(
    wheel_path: Path,
    policy_path: Path,
    target_policy: dict[str, Any],
    report: dict[str, Any],
) -> dict[str, Any]:
    if wheel_path.is_symlink() or not wheel_path.is_file():
        raise LinuxWheelError(f"wheel must be a real file: {wheel_path}")
    if policy_path.is_symlink() or not policy_path.is_file():
        raise LinuxWheelError(f"policy must be a real file: {policy_path}")
    validate_auditwheel_report(report, target_policy, wheel_path.name)
    return {
        "schema_version": 1,
        "outcome": "compatible",
        "target": target_policy["target"],
        "wheel": wheel_path.name,
        "wheel_sha256": sha256_file(wheel_path),
        "policy": policy_path.name,
        "policy_sha256": sha256_file(policy_path),
        "auditor": target_policy["auditor"],
        "auditwheel": report,
    }


def verify_auditor_environment(auditor_policy: dict[str, Any]) -> None:
    packages = {
        "auditwheel": "auditwheel_version",
        "packaging": "packaging_version",
        "pyelftools": "pyelftools_version",
    }
    for package, policy_field in packages.items():
        expected_version = require_string(
            auditor_policy,
            policy_field,
            "auditor policy",
        )
        try:
            actual_version = importlib.metadata.version(package)
        except importlib.metadata.PackageNotFoundError as error:
            raise LinuxWheelError(
                f"the pinned {package} package is not installed"
            ) from error
        if actual_version != expected_version:
            raise LinuxWheelError(
                f"{package} version {actual_version!r} does not equal "
                f"{expected_version!r}"
            )


def run_auditwheel(
    wheel_path: Path,
    auditor_policy: dict[str, Any],
) -> dict[str, Any]:
    verify_auditor_environment(auditor_policy)

    command = (
        sys.executable,
        "-m",
        "auditwheel",
        "show",
        "--json",
        str(wheel_path),
    )
    try:
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LinuxWheelError(f"cannot execute auditwheel: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise LinuxWheelError(
            f"auditwheel exited with {result.returncode}: {detail}"
        )
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise LinuxWheelError(f"auditwheel returned invalid JSON: {error}") from error
    if not isinstance(report, dict):
        raise LinuxWheelError("auditwheel JSON root must be an object")
    return report


def write_json(path: Path, payload: dict[str, Any]) -> None:
    if path.is_symlink():
        raise LinuxWheelError(f"refusing to replace symlink output: {path}")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError as error:
        raise LinuxWheelError(f"cannot write evidence {path}: {error}") from error


def main(arguments: Sequence[str] | None = None) -> int:
    options = parse_args(arguments)
    try:
        targets = load_policy(options.policy)
        target_policy = targets.get(options.target)
        if target_policy is None:
            raise LinuxWheelError(f"unknown Linux wheel target {options.target!r}")
        if options.wheel_dir.is_symlink() or not options.wheel_dir.is_dir():
            raise LinuxWheelError(
                f"wheel directory must be a real directory: {options.wheel_dir}"
            )
        wheels = sorted(options.wheel_dir.glob("*.whl"), key=lambda path: path.name)
        if len(wheels) != 1:
            raise LinuxWheelError(
                f"expected exactly one wheel in {options.wheel_dir}, found {len(wheels)}"
            )
        wheel_path = wheels[0]
        report = run_auditwheel(
            wheel_path,
            target_policy["auditor"],
        )
        evidence = build_evidence(
            wheel_path,
            options.policy,
            target_policy,
            report,
        )
        write_json(options.output, evidence)
    except LinuxWheelError as error:
        print(f"Linux wheel audit failed: {error}", file=sys.stderr)
        return 1

    print(
        f"target={options.target} wheel={wheel_path.name} "
        f"auditwheel_tag={report['overall_tag']} external_libs=0 "
        "outcome=compatible",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
