#!/usr/bin/env python3
"""Validate and package native pdfplumber Command-Line Interface binaries."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import re
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGETS_PATH = REPO_ROOT / "cli-release-targets.toml"
DEFAULT_SMOKE_POLICY_PATH = REPO_ROOT / "cli-release-smoke.toml"
DEFAULT_MANIFEST_PATH = REPO_ROOT / "crates" / "pdfplumber-cli" / "Cargo.toml"
DEFAULT_LICENSE_PATH = REPO_ROOT / "LICENSE"
DEFAULT_README_PATH = REPO_ROOT / "crates" / "pdfplumber-cli" / "README.md"
SEMVER_PATTERN = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
TARGET_KEYS = {
    "triple",
    "runner",
    "runner_os",
    "runner_arch",
    "archive_format",
    "rust_tier",
}
SMOKE_KEYS = {
    "schema_version",
    "fixture",
    "fixture_sha256",
    "expected_stdout",
    "expected_stdout_sha256",
    "args",
    "timeout_seconds",
}
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")


class CliReleaseError(RuntimeError):
    """A CLI release target, binary, or archive is invalid."""


@dataclass(frozen=True)
class ReleaseTarget:
    triple: str
    runner: str
    runner_os: str
    runner_arch: str
    archive_format: str
    rust_tier: int

    def matrix_entry(self) -> dict[str, str | int]:
        return {
            "target": self.triple,
            "runner": self.runner,
            "runner_os": self.runner_os,
            "runner_arch": self.runner_arch,
            "archive_format": self.archive_format,
            "rust_tier": self.rust_tier,
        }


@dataclass(frozen=True)
class SmokePolicy:
    fixture_path: Path
    fixture_sha256: str
    expected_stdout_path: Path
    expected_stdout_sha256: str
    args: tuple[str, ...]
    timeout_seconds: int


def require_regular_file(path: Path, description: str) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise CliReleaseError(f"{description} must be a regular file: {path}")
    try:
        content = path.read_bytes()
    except OSError as error:
        raise CliReleaseError(f"cannot read {description} {path}: {error}") from error
    if not content:
        raise CliReleaseError(f"{description} is empty: {path}")
    return content


def expected_platform(triple: str) -> tuple[str, str, str]:
    if triple == "x86_64-unknown-linux-gnu":
        return "Linux", "X64", "tar.gz"
    if triple == "aarch64-unknown-linux-gnu":
        return "Linux", "ARM64", "tar.gz"
    if triple == "x86_64-apple-darwin":
        return "macOS", "X64", "tar.gz"
    if triple == "aarch64-apple-darwin":
        return "macOS", "ARM64", "tar.gz"
    if triple == "x86_64-pc-windows-msvc":
        return "Windows", "X64", "zip"
    raise CliReleaseError(f"unsupported CLI release target: {triple}")


def parse_target(raw_target: object, position: int) -> ReleaseTarget:
    if not isinstance(raw_target, dict):
        raise CliReleaseError(f"target entry {position} must be a table")
    if set(raw_target) != TARGET_KEYS:
        missing = sorted(TARGET_KEYS - set(raw_target))
        unexpected = sorted(set(raw_target) - TARGET_KEYS)
        raise CliReleaseError(
            f"target entry {position} has invalid keys: "
            f"missing={missing}, unexpected={unexpected}"
        )
    string_keys = TARGET_KEYS - {"rust_tier"}
    for key in string_keys:
        value = raw_target[key]
        if not isinstance(value, str) or not value.strip() or value != value.strip():
            raise CliReleaseError(f"target entry {position} has invalid {key}")
    rust_tier = raw_target["rust_tier"]
    if type(rust_tier) is not int or rust_tier not in {1, 2}:
        raise CliReleaseError(f"target entry {position} must use Rust tier 1 or 2")

    triple = raw_target["triple"]
    expected_os, expected_arch, expected_archive = expected_platform(triple)
    if raw_target["runner_os"] != expected_os:
        raise CliReleaseError(f"{triple} must use runner OS {expected_os}")
    if raw_target["runner_arch"] != expected_arch:
        raise CliReleaseError(f"{triple} must use runner architecture {expected_arch}")
    if raw_target["archive_format"] != expected_archive:
        raise CliReleaseError(f"{triple} must use {expected_archive} archives")
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]*", raw_target["runner"]):
        raise CliReleaseError(f"{triple} has an invalid GitHub runner label")

    return ReleaseTarget(
        triple=triple,
        runner=raw_target["runner"],
        runner_os=raw_target["runner_os"],
        runner_arch=raw_target["runner_arch"],
        archive_format=raw_target["archive_format"],
        rust_tier=rust_tier,
    )


def load_targets(path: Path = DEFAULT_TARGETS_PATH) -> tuple[ReleaseTarget, ...]:
    try:
        source = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise CliReleaseError(
            f"cannot read CLI release target policy {path}: {error}"
        ) from error
    if set(source) != {"schema_version", "targets"}:
        raise CliReleaseError("CLI release target policy has unexpected top-level keys")
    if source["schema_version"] != 1:
        raise CliReleaseError("CLI release target policy must use schema version 1")
    raw_targets = source["targets"]
    if not isinstance(raw_targets, list) or not raw_targets:
        raise CliReleaseError("CLI release target policy must declare targets")
    targets = tuple(
        parse_target(raw_target, position)
        for position, raw_target in enumerate(raw_targets, start=1)
    )
    triples = [target.triple for target in targets]
    if len(set(triples)) != len(triples):
        raise CliReleaseError("CLI release target policy contains duplicate triples")
    return targets


def resolve_policy_file(policy_path: Path, raw_value: object, description: str) -> Path:
    if (
        not isinstance(raw_value, str)
        or not raw_value
        or raw_value != raw_value.strip()
    ):
        raise CliReleaseError(f"CLI smoke {description} must be a relative path")
    relative_path = Path(raw_value)
    if relative_path.is_absolute() or any(
        part in {"", ".", ".."} for part in relative_path.parts
    ):
        raise CliReleaseError(
            f"CLI smoke {description} must stay below the policy root"
        )
    policy_root = policy_path.parent.resolve()
    resolved_path = (policy_root / relative_path).resolve()
    try:
        resolved_path.relative_to(policy_root)
    except ValueError as error:
        raise CliReleaseError(
            f"CLI smoke {description} must stay below the policy root"
        ) from error
    return resolved_path


def require_sha256(raw_value: object, description: str) -> str:
    if not isinstance(raw_value, str) or SHA256_PATTERN.fullmatch(raw_value) is None:
        raise CliReleaseError(f"CLI smoke {description} must be lowercase SHA-256")
    return raw_value


def load_smoke_policy(path: Path = DEFAULT_SMOKE_POLICY_PATH) -> SmokePolicy:
    try:
        source = tomllib.loads(require_regular_file(path, "CLI smoke policy").decode())
    except (UnicodeError, tomllib.TOMLDecodeError) as error:
        raise CliReleaseError(
            f"cannot read CLI smoke policy {path}: {error}"
        ) from error
    if set(source) != SMOKE_KEYS:
        missing = sorted(SMOKE_KEYS - set(source))
        unexpected = sorted(set(source) - SMOKE_KEYS)
        raise CliReleaseError(
            "CLI smoke policy has invalid keys: "
            f"missing={missing}, unexpected={unexpected}"
        )
    if source["schema_version"] != 1:
        raise CliReleaseError("CLI smoke policy must use schema version 1")
    raw_args = source["args"]
    if not isinstance(raw_args, list) or not raw_args:
        raise CliReleaseError("CLI smoke args must be a non-empty array")
    if any(
        not isinstance(argument, str)
        or not argument
        or argument != argument.strip()
        or "\0" in argument
        for argument in raw_args
    ):
        raise CliReleaseError("CLI smoke args must contain non-empty strings")
    if raw_args.count("{fixture}") != 1 or any(
        "{fixture}" in argument and argument != "{fixture}" for argument in raw_args
    ):
        raise CliReleaseError(
            "CLI smoke args must contain one exact {fixture} argument"
        )
    timeout_seconds = source["timeout_seconds"]
    if type(timeout_seconds) is not int or not 1 <= timeout_seconds <= 300:
        raise CliReleaseError("CLI smoke timeout_seconds must be from 1 through 300")
    return SmokePolicy(
        fixture_path=resolve_policy_file(path, source["fixture"], "fixture"),
        fixture_sha256=require_sha256(source["fixture_sha256"], "fixture digest"),
        expected_stdout_path=resolve_policy_file(
            path, source["expected_stdout"], "expected standard output"
        ),
        expected_stdout_sha256=require_sha256(
            source["expected_stdout_sha256"], "expected standard-output digest"
        ),
        args=tuple(raw_args),
        timeout_seconds=timeout_seconds,
    )


def github_matrix(path: Path = DEFAULT_TARGETS_PATH) -> dict[str, object]:
    return {"include": [target.matrix_entry() for target in load_targets(path)]}


def validate_version(version: str) -> str:
    if SEMVER_PATTERN.fullmatch(version) is None:
        raise CliReleaseError(f"CLI package version is not strict SemVer: {version!r}")
    return version


def validate_release_tag(release_tag: str, version: str) -> str:
    version = validate_version(version)
    if release_tag and release_tag != f"v{version}":
        raise CliReleaseError(
            f"release tag {release_tag!r} does not match CLI version v{version}"
        )
    return version


def version_from_manifest(path: Path = DEFAULT_MANIFEST_PATH) -> str:
    try:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        version = manifest["package"]["version"]
    except (
        KeyError,
        TypeError,
        OSError,
        UnicodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        raise CliReleaseError(
            f"cannot read CLI package version from {path}: {error}"
        ) from error
    if not isinstance(version, str):
        raise CliReleaseError(f"CLI package version in {path} must be a string")
    return validate_version(version)


def rustc_host() -> str:
    try:
        completed = subprocess.run(
            ("rustc", "--version", "--verbose"),
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as error:
        raise CliReleaseError(f"cannot inspect the Rust host: {error}") from error
    if completed.returncode != 0:
        raise CliReleaseError("rustc host inspection failed")
    for line in completed.stdout.splitlines():
        if line.startswith("host: "):
            host = line.removeprefix("host: ").strip()
            if host:
                return host
    raise CliReleaseError("rustc did not report a host triple")


def target_policy(target: str) -> ReleaseTarget:
    for candidate in load_targets():
        if candidate.triple == target:
            return candidate
    raise CliReleaseError(f"unsupported CLI release target: {target}")


def validate_routing(
    policy: ReleaseTarget,
    runner_os: str,
    runner_arch: str,
    rustc_host_triple: str,
) -> None:
    if (runner_os, runner_arch) != (policy.runner_os, policy.runner_arch):
        raise CliReleaseError(
            f"{policy.triple} was routed to {runner_os}/{runner_arch}; "
            f"expected {policy.runner_os}/{policy.runner_arch}"
        )
    if rustc_host_triple != policy.triple:
        raise CliReleaseError(
            f"{policy.triple} must build natively; rustc host is {rustc_host_triple}"
        )


def validate_elf(content: bytes, target: str) -> None:
    if len(content) < 20 or content[:4] != b"\x7fELF":
        raise CliReleaseError(f"{target} binary is not ELF")
    if content[4:6] != b"\x02\x01":
        raise CliReleaseError(f"{target} binary must be 64-bit little-endian ELF")
    expected_machine = 62 if target.startswith("x86_64") else 183
    if struct.unpack_from("<H", content, 18)[0] != expected_machine:
        raise CliReleaseError(f"{target} binary has the wrong ELF architecture")


def validate_mach_o(content: bytes, target: str) -> None:
    if len(content) < 8 or content[:4] != b"\xcf\xfa\xed\xfe":
        raise CliReleaseError(f"{target} binary is not little-endian 64-bit Mach-O")
    expected_cpu = 0x01000007 if target.startswith("x86_64") else 0x0100000C
    if struct.unpack_from("<I", content, 4)[0] != expected_cpu:
        raise CliReleaseError(f"{target} binary has the wrong Mach-O architecture")


def validate_pe(content: bytes, target: str) -> None:
    if len(content) < 0x40 or content[:2] != b"MZ":
        raise CliReleaseError(f"{target} binary is not PE/COFF")
    pe_offset = struct.unpack_from("<I", content, 0x3C)[0]
    if pe_offset + 6 > len(content) or content[pe_offset : pe_offset + 4] != b"PE\0\0":
        raise CliReleaseError(f"{target} binary has an invalid PE header")
    if struct.unpack_from("<H", content, pe_offset + 4)[0] != 0x8664:
        raise CliReleaseError(f"{target} binary has the wrong PE architecture")


def validate_executable(content: bytes, target: str) -> None:
    if target.endswith("linux-gnu"):
        validate_elf(content, target)
    elif target.endswith("apple-darwin"):
        validate_mach_o(content, target)
    elif target.endswith("windows-msvc"):
        validate_pe(content, target)
    else:
        raise CliReleaseError(f"unsupported CLI executable format for {target}")


def tar_info(name: str, content: bytes, mode: int) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.size = len(content)
    info.mode = mode
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    return info


def write_tar_archive(path: Path, members: Sequence[tuple[str, bytes, int]]) -> None:
    try:
        with (
            path.open("wb") as raw_archive,
            gzip.GzipFile(
                filename="", mode="wb", fileobj=raw_archive, mtime=0
            ) as compressed,
            tarfile.open(
                fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT
            ) as archive,
        ):
            for name, content, mode in members:
                archive.addfile(tar_info(name, content, mode), io.BytesIO(content))
    except OSError as error:
        raise CliReleaseError(f"cannot create {path}: {error}") from error


def zip_info(name: str, mode: int) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.create_system = 3
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = mode << 16
    return info


def write_zip_archive(path: Path, members: Sequence[tuple[str, bytes, int]]) -> None:
    try:
        with zipfile.ZipFile(path, mode="w") as archive:
            for name, content, mode in members:
                archive.writestr(zip_info(name, mode), content)
    except OSError as error:
        raise CliReleaseError(f"cannot create {path}: {error}") from error


def verify_archive(
    path: Path,
    policy: ReleaseTarget,
    members: Sequence[tuple[str, bytes, int]],
) -> None:
    expected_names = [name for name, _content, _mode in members]
    try:
        if policy.archive_format == "tar.gz":
            with tarfile.open(path, "r:gz") as archive:
                if archive.getnames() != expected_names:
                    raise CliReleaseError(f"{path} has an unexpected archive layout")
                for name, content, mode in members:
                    member = archive.getmember(name)
                    extracted = archive.extractfile(member)
                    if not member.isfile() or member.mode != mode or extracted is None:
                        raise CliReleaseError(f"{path} has invalid member {name}")
                    if extracted.read() != content:
                        raise CliReleaseError(f"{path} changed member {name}")
        else:
            with zipfile.ZipFile(path) as archive:
                if archive.namelist() != expected_names:
                    raise CliReleaseError(f"{path} has an unexpected archive layout")
                for name, content, mode in members:
                    member = archive.getinfo(name)
                    if (
                        member.external_attr >> 16 != mode
                        or archive.read(member) != content
                    ):
                        raise CliReleaseError(f"{path} has invalid member {name}")
    except (OSError, tarfile.TarError, zipfile.BadZipFile) as error:
        raise CliReleaseError(f"cannot verify {path}: {error}") from error


def release_archive_name(target: str, version: str) -> str:
    policy = target_policy(target)
    version = validate_version(version)
    suffix = ".zip" if policy.archive_format == "zip" else ".tar.gz"
    return f"pdfplumber-cli-{version}-{target}{suffix}"


def read_archive_executable(archive_path: Path, target: str, version: str) -> bytes:
    policy = target_policy(target)
    expected_archive_name = release_archive_name(target, version)
    if archive_path.name != expected_archive_name:
        raise CliReleaseError(
            f"CLI smoke archive must be named {expected_archive_name}"
        )
    require_regular_file(archive_path, "CLI release archive")
    archive_root = expected_archive_name.removesuffix(
        ".zip" if policy.archive_format == "zip" else ".tar.gz"
    )
    executable_name = (
        "pdfplumber.exe" if target.endswith("windows-msvc") else "pdfplumber"
    )
    executable_member_name = f"{archive_root}/{executable_name}"
    expected_member_names = [
        executable_member_name,
        f"{archive_root}/README.md",
        f"{archive_root}/LICENSE",
    ]
    executable: bytes | None = None
    try:
        if policy.archive_format == "tar.gz":
            with tarfile.open(archive_path, "r:gz") as archive:
                if archive.getnames() != expected_member_names:
                    raise CliReleaseError(
                        f"{archive_path} has an unexpected smoke archive layout"
                    )
                for member_name in expected_member_names:
                    member = archive.getmember(member_name)
                    extracted = archive.extractfile(member)
                    expected_mode = (
                        0o755 if member_name == executable_member_name else 0o644
                    )
                    if (
                        not member.isfile()
                        or member.mode != expected_mode
                        or extracted is None
                    ):
                        raise CliReleaseError(
                            f"{archive_path} has invalid member {member_name}"
                        )
                    content = extracted.read()
                    if not content:
                        raise CliReleaseError(
                            f"{archive_path} has empty member {member_name}"
                        )
                    if member_name == executable_member_name:
                        executable = content
        else:
            with zipfile.ZipFile(archive_path) as archive:
                if archive.namelist() != expected_member_names:
                    raise CliReleaseError(
                        f"{archive_path} has an unexpected smoke archive layout"
                    )
                for member_name in expected_member_names:
                    member = archive.getinfo(member_name)
                    expected_mode = (
                        0o100755 if member_name == executable_member_name else 0o100644
                    )
                    content = archive.read(member)
                    if (
                        member.is_dir()
                        or member.external_attr >> 16 != expected_mode
                        or not content
                    ):
                        raise CliReleaseError(
                            f"{archive_path} has invalid member {member_name}"
                        )
                    if member_name == executable_member_name:
                        executable = content
    except (KeyError, OSError, tarfile.TarError, zipfile.BadZipFile) as error:
        raise CliReleaseError(
            f"cannot inspect smoke archive {archive_path}: {error}"
        ) from error
    if executable is None:
        raise CliReleaseError(f"{archive_path} omitted the smoke executable")
    validate_executable(executable, target)
    return executable


def sha256(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


SmokeRunner = Callable[..., subprocess.CompletedProcess[bytes]]


def smoke_release(
    *,
    archive_path: Path,
    target: str,
    version: str,
    policy: SmokePolicy,
    runner: SmokeRunner | None = None,
) -> None:
    fixture = require_regular_file(policy.fixture_path, "CLI smoke fixture")
    if sha256(fixture) != policy.fixture_sha256:
        raise CliReleaseError("CLI smoke fixture SHA-256 does not match policy")
    expected_stdout = require_regular_file(
        policy.expected_stdout_path, "CLI smoke expected standard output"
    )
    if sha256(expected_stdout) != policy.expected_stdout_sha256:
        raise CliReleaseError(
            "CLI smoke expected standard-output SHA-256 does not match policy"
        )
    executable = read_archive_executable(archive_path, target, version)
    executable_name = (
        "pdfplumber.exe" if target.endswith("windows-msvc") else "pdfplumber"
    )
    arguments = tuple(
        str(policy.fixture_path.resolve()) if argument == "{fixture}" else argument
        for argument in policy.args
    )
    execution_runner = runner or subprocess.run
    try:
        with tempfile.TemporaryDirectory(prefix="pdfplumber-cli-smoke-") as directory:
            executable_path = Path(directory) / executable_name
            executable_path.write_bytes(executable)
            executable_path.chmod(0o755)
            completed = execution_runner(
                (str(executable_path), *arguments),
                check=False,
                capture_output=True,
                timeout=policy.timeout_seconds,
            )
    except subprocess.TimeoutExpired as error:
        raise CliReleaseError(
            f"{target} smoke exceeded {policy.timeout_seconds} seconds"
        ) from error
    except OSError as error:
        raise CliReleaseError(
            f"cannot execute {target} smoke binary: {error}"
        ) from error
    if completed.returncode != 0:
        raise CliReleaseError(
            f"{target} smoke returned exit code {completed.returncode}"
        )
    if completed.stderr:
        raise CliReleaseError(
            f"{target} smoke wrote {len(completed.stderr)} bytes to standard error"
        )
    if completed.stdout != expected_stdout:
        raise CliReleaseError(
            f"{target} smoke standard output did not match policy: "
            f"expected_sha256={policy.expected_stdout_sha256} "
            f"actual_sha256={sha256(completed.stdout)}"
        )


def package_release(
    *,
    binary_path: Path,
    target: str,
    runner_os: str,
    runner_arch: str,
    rustc_host: str,
    version: str,
    output_dir: Path,
    license_path: Path,
    readme_path: Path,
) -> Path:
    policy = target_policy(target)
    validate_routing(policy, runner_os, runner_arch, rustc_host)
    version = validate_version(version)
    binary = require_regular_file(binary_path, "CLI executable")
    validate_executable(binary, target)
    license_content = require_regular_file(license_path, "license")
    readme_content = require_regular_file(readme_path, "CLI README")

    executable_name = (
        "pdfplumber.exe" if target.endswith("windows-msvc") else "pdfplumber"
    )
    archive_root = f"pdfplumber-cli-{version}-{target}"
    suffix = ".zip" if policy.archive_format == "zip" else ".tar.gz"
    archive_path = output_dir / f"{archive_root}{suffix}"
    members = (
        (f"{archive_root}/{executable_name}", binary, 0o100755),
        (f"{archive_root}/README.md", readme_content, 0o100644),
        (f"{archive_root}/LICENSE", license_content, 0o100644),
    )
    try:
        output_dir.mkdir(parents=True, exist_ok=True)
        if archive_path.is_symlink():
            raise CliReleaseError(f"refusing to replace symlink {archive_path}")
        archive_path.unlink(missing_ok=True)
    except OSError as error:
        raise CliReleaseError(
            f"cannot prepare archive path {archive_path}: {error}"
        ) from error

    archive_members = tuple(
        (name, content, mode & 0o777) for name, content, mode in members
    )
    if policy.archive_format == "zip":
        write_zip_archive(archive_path, members)
    else:
        write_tar_archive(archive_path, archive_members)
    verify_archive(
        archive_path,
        policy,
        members if policy.archive_format == "zip" else archive_members,
    )
    return archive_path


CargoMetadataRunner = Callable[..., subprocess.CompletedProcess[str]]


def cargo_target_directory(
    runner: CargoMetadataRunner = subprocess.run,
) -> Path:
    command = ("cargo", "metadata", "--no-deps", "--format-version", "1")
    try:
        completed = runner(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as error:
        raise CliReleaseError(
            f"cannot inspect Cargo's target directory: {error}"
        ) from error
    if completed.returncode != 0:
        raise CliReleaseError("Cargo target-directory inspection failed")
    try:
        metadata = json.loads(completed.stdout)
        raw_target_directory = metadata["target_directory"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise CliReleaseError("Cargo metadata omitted target_directory") from error
    if not isinstance(raw_target_directory, str) or not raw_target_directory:
        raise CliReleaseError("Cargo metadata target_directory must be a path")
    target_directory = Path(raw_target_directory)
    if not target_directory.is_absolute():
        raise CliReleaseError("Cargo metadata target_directory must be absolute")
    return target_directory


def default_binary_path(target: str, target_directory: Path | None = None) -> Path:
    executable = "pdfplumber.exe" if target.endswith("windows-msvc") else "pdfplumber"
    directory = target_directory or cargo_target_directory()
    return directory / target / "release" / executable


def write_github_matrix(output_path: Path | None) -> None:
    matrix = json.dumps(github_matrix(), separators=(",", ":"), sort_keys=True)
    if output_path is None:
        print(matrix)
        return
    try:
        with output_path.open("a", encoding="utf-8") as output:
            output.write(f"matrix={matrix}\n")
    except OSError as error:
        raise CliReleaseError(
            f"cannot write GitHub matrix output {output_path}: {error}"
        ) from error
    print(f"validated target_matrix={len(load_targets())} outcome=ready", flush=True)


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    matrix_parser = subparsers.add_parser("matrix", help="export the target matrix")
    matrix_parser.add_argument("--github-output", type=Path)

    package_parser = subparsers.add_parser("package", help="package one native binary")
    package_parser.add_argument("--target", required=True)
    package_parser.add_argument("--runner-os", required=True)
    package_parser.add_argument("--runner-arch", required=True)
    package_parser.add_argument("--release-tag", default="")
    package_parser.add_argument("--binary", type=Path)
    package_parser.add_argument("--output-dir", type=Path, default=REPO_ROOT / "dist")
    package_parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST_PATH)
    package_parser.add_argument("--license", type=Path, default=DEFAULT_LICENSE_PATH)
    package_parser.add_argument("--readme", type=Path, default=DEFAULT_README_PATH)

    smoke_parser = subparsers.add_parser(
        "smoke", help="execute one packaged native binary against the smoke fixture"
    )
    smoke_parser.add_argument("--target", required=True)
    smoke_parser.add_argument("--release-tag", default="")
    archive_group = smoke_parser.add_mutually_exclusive_group()
    archive_group.add_argument("--archive", type=Path)
    archive_group.add_argument("--archive-dir", type=Path, default=REPO_ROOT / "dist")
    smoke_parser.add_argument("--policy", type=Path, default=DEFAULT_SMOKE_POLICY_PATH)
    smoke_parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST_PATH)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = create_parser().parse_args(argv)
    try:
        if arguments.command == "matrix":
            write_github_matrix(arguments.github_output)
            return 0

        version = version_from_manifest(arguments.manifest)
        validate_release_tag(arguments.release_tag, version)
        if arguments.command == "smoke":
            archive_path = arguments.archive or (
                arguments.archive_dir / release_archive_name(arguments.target, version)
            )
            smoke_policy = load_smoke_policy(arguments.policy)
            smoke_release(
                archive_path=archive_path,
                target=arguments.target,
                version=version,
                policy=smoke_policy,
            )
            print(
                f"target={arguments.target} version={version} outcome=smoke-passed "
                f"fixture_sha256={smoke_policy.fixture_sha256} "
                f"stdout_sha256={smoke_policy.expected_stdout_sha256}",
                flush=True,
            )
            return 0

        binary_path = arguments.binary or default_binary_path(arguments.target)
        archive = package_release(
            binary_path=binary_path,
            target=arguments.target,
            runner_os=arguments.runner_os,
            runner_arch=arguments.runner_arch,
            rustc_host=rustc_host(),
            version=version,
            output_dir=arguments.output_dir,
            license_path=arguments.license,
            readme_path=arguments.readme,
        )
        print(
            f"target={arguments.target} version={version} "
            f"outcome=packaged archive={archive.name}",
            flush=True,
        )
        return 0
    except CliReleaseError as error:
        print(f"CLI release command failed: {error}", file=sys.stderr, flush=True)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
