#!/usr/bin/env python3
"""Verify one Windows wheel's PE imports and installed path behavior."""

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


class WindowsWheelError(ValueError):
    """Raised when a Windows wheel violates the release policy."""


Runner = Callable[..., subprocess.CompletedProcess[str]]


INSTALLED_PROBE = r"""
import hashlib
import importlib.metadata
import json
import platform
import sys
import tempfile
from pathlib import Path

import pdfplumber
from pdfplumber import _native

fixture = Path(sys.argv[1]).resolve()
expected_path = Path(sys.argv[2]).resolve()
minimum_long_path_characters = int(sys.argv[3])
expected_records = [
    json.loads(line)
    for line in expected_path.read_text(encoding="utf-8").splitlines()
    if line.strip()
]
if len(expected_records) != 1 or expected_records[0].get("page") != 1:
    raise RuntimeError("expected-output policy must contain exactly page 1")
expected_text = expected_records[0].get("text")
fixture_bytes = fixture.read_bytes()
fixture_sha256 = hashlib.sha256(fixture_bytes).hexdigest()


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as input_file:
        for chunk in iter(lambda: input_file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def probe_path(case, path):
    rendered = str(path)
    if rendered.startswith("\\\\?\\"):
        raise RuntimeError("path probe must use a normal Win32 path")
    if rendered.isascii():
        raise RuntimeError("path probe does not contain non-ASCII characters")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(fixture_bytes)
    with pdfplumber.open(path) as document:
        texts = [page.extract_text() for page in document.pages]
    if texts != [expected_text]:
        raise RuntimeError(f"{case} path extraction does not match exact text")
    return {
        "case": case,
        "contains_non_ascii": True,
        "normal_win32_path": True,
        "path_characters": len(rendered),
        "page_count": len(texts),
        "text_sha256": hashlib.sha256(texts[0].encode("utf-8")).hexdigest(),
        "fixture_sha256": fixture_sha256,
    }


with tempfile.TemporaryDirectory(prefix="pdfplumber-rs-windows-paths-") as temporary:
    root = Path(temporary).resolve()
    unicode_path = root / "資料-테스트-данные" / "基本-텍스트.pdf"

    long_directory = root / "長い-경로-длинный"
    long_filename = "基本-텍스트-данные.pdf"
    while len(str(long_directory / long_filename)) < minimum_long_path_characters:
        long_directory /= "階層-디렉터리-каталог-0123456789"
    long_path = long_directory / long_filename

    path_probes = [
        probe_path("unicode", unicode_path),
        probe_path("long_unicode", long_path),
    ]

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
            "native_module_sha256": sha256_file(native_path),
            "expected_sha256": sha256_file(expected_path),
            "path_probes": path_probes,
        },
        sort_keys=True,
    )
)
"""


def parse_args(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Verify one Windows wheel's filename, PE dependencies, native host, "
            "and installed Unicode and long-path extraction."
        )
    )
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--wheel-dir", type=Path, required=True)
    parser.add_argument("--dumpbin", type=Path, required=True)
    parser.add_argument("--installed-python", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(arguments)


def read_toml(path: Path) -> dict[str, Any]:
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as error:
        raise WindowsWheelError(f"cannot read policy {path}: {error}") from error
    try:
        return tomllib.loads(content)
    except tomllib.TOMLDecodeError as error:
        raise WindowsWheelError(f"invalid policy TOML {path}: {error}") from error


def require_string(mapping: dict[str, Any], field: str, context: str) -> str:
    value = mapping.get(field)
    if not isinstance(value, str) or not value:
        raise WindowsWheelError(f"{context} requires a non-empty {field}")
    return value


def require_integer(mapping: dict[str, Any], field: str, context: str) -> int:
    value = mapping.get(field)
    if not isinstance(value, int) or isinstance(value, bool):
        raise WindowsWheelError(f"{context} requires an integer {field}")
    return value


def load_policy(path: Path) -> dict[str, dict[str, Any]]:
    policy = read_toml(path)
    if policy.get("schema_version") != 1:
        raise WindowsWheelError("Windows wheel policy requires schema_version = 1")

    raw_targets = policy.get("targets")
    if not isinstance(raw_targets, list) or not raw_targets:
        raise WindowsWheelError("Windows wheel policy requires at least one target")

    string_fields = (
        "target",
        "runner",
        "runner_arch",
        "machine",
        "python_tag",
        "abi_tag",
        "platform_tag",
        "pe_machine",
        "pe_format",
    )
    targets: dict[str, dict[str, Any]] = {}
    for index, raw_target in enumerate(raw_targets):
        context = f"target policy #{index + 1}"
        if not isinstance(raw_target, dict):
            raise WindowsWheelError(f"{context} must be a table")
        target: dict[str, Any] = {
            field: require_string(raw_target, field, context)
            for field in string_fields
        }
        target["long_paths_enabled"] = require_integer(
            raw_target, "long_paths_enabled", context
        )
        target["minimum_long_path_characters"] = require_integer(
            raw_target, "minimum_long_path_characters", context
        )
        dependencies = raw_target.get("required_dependencies")
        if not isinstance(dependencies, list) or not all(
            isinstance(dependency, str) and dependency for dependency in dependencies
        ):
            raise WindowsWheelError(
                f"{context} requires a non-empty required_dependencies list"
            )
        normalized = sorted({dependency.lower() for dependency in dependencies})
        if dependencies != normalized:
            raise WindowsWheelError(
                f"{context} required_dependencies must be unique, lowercase, and sorted"
            )
        target["required_dependencies"] = dependencies
        if target["long_paths_enabled"] != 1:
            raise WindowsWheelError(f"{context} must require LongPathsEnabled=1")
        if target["minimum_long_path_characters"] <= 260:
            raise WindowsWheelError(
                f"{context} long-path threshold must exceed the 260-character limit"
            )
        target_name = target["target"]
        if target_name in targets:
            raise WindowsWheelError(f"Windows wheel policy repeats target {target_name}")
        targets[target_name] = target
    return targets


def validate_host(
    target_policy: dict[str, Any],
    system: str,
    machine: str,
    runner_arch: str,
) -> None:
    if system != "Windows":
        raise WindowsWheelError(
            f"wheel verification requires Windows, found {system!r}"
        )
    if machine != target_policy["machine"]:
        raise WindowsWheelError(
            f"native runner machine {machine!r} does not equal "
            f"{target_policy['machine']!r}"
        )
    if runner_arch != target_policy["runner_arch"]:
        raise WindowsWheelError(
            f"RUNNER_ARCH {runner_arch!r} does not equal "
            f"{target_policy['runner_arch']!r}"
        )


def validate_wheel_name(wheel_name: str, target_policy: dict[str, Any]) -> None:
    expected_suffix = (
        f"-{target_policy['python_tag']}-{target_policy['abi_tag']}-"
        f"{target_policy['platform_tag']}.whl"
    )
    if not wheel_name.endswith(expected_suffix):
        raise WindowsWheelError(
            f"wheel filename does not carry exact policy tag {expected_suffix}: "
            f"{wheel_name}"
        )


def parse_dumpbin(output: str) -> dict[str, Any]:
    machines = re.findall(
        r"(?mi)^\s*([0-9a-f]{3,4})\s+machine\s+\(x64\)\s*$", output
    )
    if len(machines) != 1:
        raise WindowsWheelError("DUMPBIN must report exactly one x64 PE machine")
    formats = re.findall(
        r"(?mi)^\s*[0-9a-f]+\s+magic\s+#\s+\((PE32\+)\)\s*$", output
    )
    if len(formats) != 1:
        raise WindowsWheelError("DUMPBIN must report exactly one PE32+ format")
    dependencies = sorted(
        {
            dependency.lower()
            for dependency in re.findall(
                r"(?mi)^\s*([a-z0-9][a-z0-9._+-]*\.dll)\s*$", output
            )
        }
    )
    if not dependencies:
        raise WindowsWheelError("DUMPBIN did not report any imported DLLs")
    return {
        "pe_machine": machines[0].upper(),
        "pe_format": formats[0].upper(),
        "dependencies": dependencies,
    }


def validate_pe(inspection: dict[str, Any], target_policy: dict[str, Any]) -> None:
    for field in ("pe_machine", "pe_format", "dependencies"):
        if inspection.get(field) != target_policy.get(
            "required_dependencies" if field == "dependencies" else field
        ):
            expected_field = (
                "required_dependencies" if field == "dependencies" else field
            )
            raise WindowsWheelError(
                f"PE {field} {inspection.get(field)!r} does not equal "
                f"{target_policy[expected_field]!r}"
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
        raise WindowsWheelError(f"cannot execute {command[0]}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise WindowsWheelError(
            f"{command[0]} exited with {result.returncode}: {detail}"
        )
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
        raise WindowsWheelError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def inspect_native_module(
    native_path: Path,
    dumpbin: Path,
    target_policy: dict[str, Any],
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    inspection = parse_dumpbin(
        run_tool(
            (str(dumpbin), "/NOLOGO", "/HEADERS", "/DEPENDENTS", str(native_path)),
            runner,
        )
    )
    validate_pe(inspection, target_policy)
    return inspection


def inspect_wheel(
    wheel_path: Path,
    dumpbin: Path,
    target_policy: dict[str, Any],
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    if wheel_path.is_symlink() or not wheel_path.is_file():
        raise WindowsWheelError(f"wheel must be a real file: {wheel_path}")
    if dumpbin.is_symlink() or not dumpbin.is_file():
        raise WindowsWheelError(f"DUMPBIN must be a real file: {dumpbin}")
    validate_wheel_name(wheel_path.name, target_policy)
    try:
        with zipfile.ZipFile(wheel_path) as archive:
            native_names = [
                name
                for name in archive.namelist()
                if name.startswith("pdfplumber/_native") and name.endswith(".pyd")
            ]
            if len(native_names) != 1:
                raise WindowsWheelError(
                    "wheel must contain exactly one private Windows native module"
                )
            native_name = native_names[0]
            archive_path = PurePosixPath(native_name)
            if archive_path.is_absolute() or ".." in archive_path.parts:
                raise WindowsWheelError(
                    f"unsafe native module path in wheel: {native_name}"
                )
            native_bytes = archive.read(native_name)
    except (OSError, zipfile.BadZipFile, KeyError) as error:
        raise WindowsWheelError(f"cannot inspect wheel {wheel_path}: {error}") from error

    with tempfile.TemporaryDirectory() as temporary_directory:
        native_path = Path(temporary_directory) / "native-module.pyd"
        native_path.write_bytes(native_bytes)
        inspection = inspect_native_module(
            native_path, dumpbin, target_policy, runner=runner
        )
    return {
        "native_module": native_name,
        "native_module_sha256": sha256_bytes(native_bytes),
        **inspection,
    }


def read_long_paths_enabled() -> int:
    try:
        import winreg

        with winreg.OpenKey(
            winreg.HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\FileSystem",
        ) as key:
            value, _value_type = winreg.QueryValueEx(key, "LongPathsEnabled")
    except (ImportError, OSError) as error:
        raise WindowsWheelError(f"cannot read LongPathsEnabled: {error}") from error
    if not isinstance(value, int):
        raise WindowsWheelError("LongPathsEnabled is not an integer registry value")
    return value


def validate_long_paths_enabled(
    observed: int, target_policy: dict[str, Any]
) -> None:
    expected = target_policy["long_paths_enabled"]
    if observed != expected:
        raise WindowsWheelError(
            f"LongPathsEnabled={observed!r} does not equal required value {expected}"
        )


def _require_probe_string(probe: dict[str, Any], field: str) -> str:
    value = probe.get(field)
    if not isinstance(value, str) or not value:
        raise WindowsWheelError(f"installed probe requires {field}")
    return value


def validate_installed_probe(
    probe: dict[str, Any],
    target_policy: dict[str, Any],
    native_module_sha256: str,
) -> None:
    if probe.get("machine") != target_policy["machine"]:
        raise WindowsWheelError("installed probe ran on the wrong machine architecture")
    python_version = probe.get("python_version")
    if not isinstance(python_version, str) or not python_version.startswith("3.13."):
        raise WindowsWheelError("installed probe did not run on CPython 3.13")
    for field in (
        "distribution_version",
        "package_file",
        "native_module_file",
        "native_module_sha256",
        "expected_sha256",
    ):
        _require_probe_string(probe, field)
    if "lib/site-packages/pdfplumber/" not in probe["package_file"].lower():
        raise WindowsWheelError("installed package was not imported from site-packages")
    if "lib/site-packages/pdfplumber/_native" not in probe[
        "native_module_file"
    ].lower():
        raise WindowsWheelError(
            "installed native module was not imported from site-packages"
        )
    if probe["native_module_sha256"] != native_module_sha256:
        raise WindowsWheelError(
            "installed native module digest does not equal the wheel module digest"
        )

    path_probes = probe.get("path_probes")
    if not isinstance(path_probes, list) or len(path_probes) != 2:
        raise WindowsWheelError("installed probe requires exactly two path probes")
    if not all(isinstance(path_probe, dict) for path_probe in path_probes):
        raise WindowsWheelError("installed path probes must be JSON objects")
    cases = [path_probe.get("case") for path_probe in path_probes]
    if cases != ["unicode", "long_unicode"]:
        raise WindowsWheelError(
            "installed probe requires ordered unicode and long_unicode cases"
        )
    for path_probe in path_probes:
        case = path_probe["case"]
        if path_probe.get("contains_non_ascii") is not True:
            raise WindowsWheelError(f"{case} probe did not contain non-ASCII text")
        if path_probe.get("normal_win32_path") is not True:
            raise WindowsWheelError(f"{case} probe did not use a normal Win32 path")
        if path_probe.get("page_count") != 1:
            raise WindowsWheelError(f"{case} probe did not extract exactly one page")
        path_characters = path_probe.get("path_characters")
        if (
            not isinstance(path_characters, int)
            or isinstance(path_characters, bool)
            or path_characters <= 0
        ):
            raise WindowsWheelError(f"{case} probe has an invalid path length")
        for field in ("text_sha256", "fixture_sha256"):
            value = path_probe.get(field)
            if not isinstance(value, str) or not value:
                raise WindowsWheelError(f"{case} probe requires {field}")
    if path_probes[1]["path_characters"] < target_policy[
        "minimum_long_path_characters"
    ]:
        raise WindowsWheelError("long_unicode probe did not reach the policy length")


def run_installed_probe(
    installed_python: Path,
    fixture: Path,
    expected: Path,
    target_policy: dict[str, Any],
    native_module_sha256: str,
    runner: Runner = subprocess.run,
) -> dict[str, Any]:
    for path, label in (
        (installed_python, "installed Python"),
        (fixture, "fixture"),
        (expected, "expected output"),
    ):
        if not path.is_file():
            raise WindowsWheelError(f"{label} does not exist: {path}")
    output = run_tool(
        (
            str(installed_python),
            "-I",
            "-c",
            INSTALLED_PROBE,
            str(fixture),
            str(expected),
            str(target_policy["minimum_long_path_characters"]),
        ),
        runner,
    )
    try:
        probe = json.loads(output)
    except json.JSONDecodeError as error:
        raise WindowsWheelError(
            f"installed probe returned invalid JSON: {error}"
        ) from error
    if not isinstance(probe, dict):
        raise WindowsWheelError("installed probe JSON root must be an object")
    validate_installed_probe(probe, target_policy, native_module_sha256)
    if probe["expected_sha256"] != sha256_file(expected):
        raise WindowsWheelError(
            "installed probe expected-output digest does not match the input"
        )
    fixture_sha256 = sha256_file(fixture)
    for path_probe in probe["path_probes"]:
        if path_probe["fixture_sha256"] != fixture_sha256:
            raise WindowsWheelError(
                f"{path_probe['case']} probe fixture digest does not match the input"
            )
    return probe


def build_evidence(
    wheel_path: Path,
    policy_path: Path,
    target_policy: dict[str, Any],
    inspection: dict[str, Any],
    long_paths_enabled: int,
    probe: dict[str, Any],
) -> dict[str, Any]:
    if policy_path.is_symlink() or not policy_path.is_file():
        raise WindowsWheelError(f"policy must be a real file: {policy_path}")
    validate_wheel_name(wheel_path.name, target_policy)
    validate_pe(inspection, target_policy)
    validate_long_paths_enabled(long_paths_enabled, target_policy)
    native_module_sha256 = inspection.get("native_module_sha256")
    if not isinstance(native_module_sha256, str) or not native_module_sha256:
        raise WindowsWheelError("PE inspection requires native_module_sha256")
    validate_installed_probe(probe, target_policy, native_module_sha256)
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
        "long_paths_enabled": long_paths_enabled,
        "pe": inspection,
        "installed_probe": probe,
    }


def write_json(path: Path, payload: dict[str, Any]) -> None:
    if path.is_symlink():
        raise WindowsWheelError(f"refusing to replace symlink output: {path}")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError as error:
        raise WindowsWheelError(f"cannot write evidence {path}: {error}") from error


def main(arguments: Sequence[str] | None = None) -> int:
    options = parse_args(arguments)
    try:
        targets = load_policy(options.policy)
        target_policy = targets.get(options.target)
        if target_policy is None:
            raise WindowsWheelError(
                f"unknown Windows wheel target {options.target!r}"
            )
        validate_host(
            target_policy,
            platform.system(),
            platform.machine(),
            os.environ.get("RUNNER_ARCH", ""),
        )
        long_paths_enabled = read_long_paths_enabled()
        validate_long_paths_enabled(long_paths_enabled, target_policy)
        if options.wheel_dir.is_symlink() or not options.wheel_dir.is_dir():
            raise WindowsWheelError(
                f"wheel directory must be a real directory: {options.wheel_dir}"
            )
        wheels = sorted(options.wheel_dir.glob("*.whl"), key=lambda path: path.name)
        if len(wheels) != 1:
            raise WindowsWheelError(
                f"expected exactly one wheel in {options.wheel_dir}, found {len(wheels)}"
            )
        wheel_path = wheels[0]
        inspection = inspect_wheel(wheel_path, options.dumpbin, target_policy)
        probe = run_installed_probe(
            options.installed_python,
            options.fixture,
            options.expected,
            target_policy,
            inspection["native_module_sha256"],
        )
        evidence = build_evidence(
            wheel_path,
            options.policy,
            target_policy,
            inspection,
            long_paths_enabled,
            probe,
        )
        write_json(options.output, evidence)
    except WindowsWheelError as error:
        print(f"Windows wheel verification failed: {error}", file=sys.stderr)
        return 1

    print(
        f"target={options.target} wheel={wheel_path.name} "
        f"machine={inspection['pe_machine']} format={inspection['pe_format']} "
        f"dependencies={len(inspection['dependencies'])} "
        f"long_path_characters={probe['path_probes'][1]['path_characters']} "
        f"installed_version={probe['distribution_version']} outcome=compatible",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
