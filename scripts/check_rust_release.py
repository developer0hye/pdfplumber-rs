#!/usr/bin/env python3
"""Detect coordinated Rust release PRs and validate breaking-change notes."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
PUBLISHABLE_MANIFESTS = {
    "pdfplumber-core": Path("crates/pdfplumber-core/Cargo.toml"),
    "pdfplumber-parse": Path("crates/pdfplumber-parse/Cargo.toml"),
    "pdfplumber": Path("crates/pdfplumber/Cargo.toml"),
    "pdfplumber-cli": Path("crates/pdfplumber-cli/Cargo.toml"),
}
SEMVER_LIBRARY_PACKAGES = (
    "pdfplumber-core",
    "pdfplumber-parse",
    "pdfplumber",
)
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
PLACEHOLDER_WORDS = (
    "tbd",
    "todo",
    "to be documented",
    "coming soon",
    "details to follow",
)
MIGRATION_GUIDANCE_PATTERN = re.compile(
    r"\b(use|replace|rename|call|select|switch|migrate|move|pin|convert|render|update)\b",
    re.IGNORECASE,
)


class ReleaseError(ValueError):
    """Raised when the Rust release pull-request contract is invalid."""


@dataclass(frozen=True)
class ReleaseInfo:
    """A coordinated version transition for every publishable Rust package."""

    base_version: str
    release_version: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--base-rev", required=True)
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--require-migration-notes", action="store_true")
    return parser.parse_args()


def load_manifest(text: str, context: str) -> dict[str, Any]:
    try:
        manifest = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise ReleaseError(f"cannot parse {context}: {error}") from error
    package = manifest.get("package")
    if not isinstance(package, dict):
        raise ReleaseError(f"{context} has no [package] table")
    return package


def load_workspace_package(text: str, context: str) -> dict[str, Any]:
    try:
        manifest = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise ReleaseError(f"cannot parse {context}: {error}") from error
    package = manifest.get("workspace", {}).get("package")
    if not isinstance(package, dict):
        raise ReleaseError(f"{context} has no [workspace.package] table")
    return package


def read_current_manifest(repo_root: Path, relative: Path) -> dict[str, Any]:
    path = repo_root / relative
    try:
        return load_manifest(path.read_text(encoding="utf-8"), str(relative))
    except OSError as error:
        raise ReleaseError(f"cannot read {relative}: {error}") from error


def read_baseline_manifest(
    repo_root: Path, base_revision: str, relative: Path
) -> dict[str, Any]:
    result = subprocess.run(
        ("git", "show", f"{base_revision}:{relative.as_posix()}"),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or "git show failed"
        raise ReleaseError(f"cannot read baseline {relative}: {detail}")
    return load_manifest(result.stdout, f"{base_revision}:{relative}")


def read_current_workspace_package(repo_root: Path) -> dict[str, Any]:
    path = repo_root / "Cargo.toml"
    try:
        return load_workspace_package(path.read_text(encoding="utf-8"), "Cargo.toml")
    except OSError as error:
        raise ReleaseError(f"cannot read Cargo.toml: {error}") from error


def read_baseline_workspace_package(
    repo_root: Path, base_revision: str
) -> dict[str, Any]:
    result = subprocess.run(
        ("git", "show", f"{base_revision}:Cargo.toml"),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or "git show failed"
        raise ReleaseError(f"cannot read baseline Cargo.toml: {detail}")
    return load_workspace_package(result.stdout, f"{base_revision}:Cargo.toml")


def package_version(
    package: dict[str, Any],
    workspace_package: dict[str, Any],
    expected_name: str,
    context: str,
) -> str:
    if package.get("name") != expected_name:
        raise ReleaseError(f"{context} package name must be {expected_name!r}")
    version = package.get("version")
    if version == {"workspace": True}:
        version = workspace_package.get("version")
    if not isinstance(version, str) or VERSION_PATTERN.fullmatch(version) is None:
        raise ReleaseError(f"{context} version must use X.Y.Z")
    return version


def version_tuple(version: str) -> tuple[int, int, int]:
    major, minor, patch = version.split(".")
    return int(major), int(minor), int(patch)


def detect_release(repo_root: Path, base_revision: str) -> ReleaseInfo | None:
    base_versions: dict[str, str] = {}
    current_versions: dict[str, str] = {}
    base_workspace = read_baseline_workspace_package(repo_root, base_revision)
    current_workspace = read_current_workspace_package(repo_root)
    for package_name, relative in PUBLISHABLE_MANIFESTS.items():
        base_versions[package_name] = package_version(
            read_baseline_manifest(repo_root, base_revision, relative),
            base_workspace,
            package_name,
            f"baseline {relative}",
        )
        current_versions[package_name] = package_version(
            read_current_manifest(repo_root, relative),
            current_workspace,
            package_name,
            str(relative),
        )

    changed_packages = {
        name
        for name in PUBLISHABLE_MANIFESTS
        if current_versions[name] != base_versions[name]
    }
    if not changed_packages:
        return None
    if changed_packages != set(PUBLISHABLE_MANIFESTS):
        unchanged = sorted(set(PUBLISHABLE_MANIFESTS) - changed_packages)
        raise ReleaseError(
            "release PR must update all publishable Rust package versions together; "
            f"unchanged: {', '.join(unchanged)}"
        )

    unique_base_versions = set(base_versions.values())
    unique_release_versions = set(current_versions.values())
    if len(unique_base_versions) != 1 or len(unique_release_versions) != 1:
        raise ReleaseError(
            "publishable Rust packages must share one base version and one release version"
        )

    base_version = unique_base_versions.pop()
    release_version = unique_release_versions.pop()
    if version_tuple(release_version) <= version_tuple(base_version):
        raise ReleaseError(
            f"Rust release version {release_version} must exceed {base_version}"
        )
    return ReleaseInfo(base_version=base_version, release_version=release_version)


def changelog_release_body(changelog: str, version: str) -> str:
    match = re.search(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}\n"
        r"(?P<body>.*?)(?=^## |\Z)",
        changelog,
        re.MULTILINE | re.DOTALL,
    )
    return "" if match is None else match.group("body")


def validate_migration_notes(repo_root: Path, release_version: str) -> None:
    changelog_path = repo_root / "CHANGELOG.md"
    try:
        changelog = changelog_path.read_text(encoding="utf-8")
    except OSError as error:
        raise ReleaseError(f"cannot read CHANGELOG.md: {error}") from error

    release_body = changelog_release_body(changelog, release_version)
    note_matches = re.finditer(
        r"^- \*\*Migration:\*\* Breaking:\s+(?P<note>.*?)"
        r"(?=^- |^### |^## |\Z)",
        release_body,
        re.MULTILINE | re.DOTALL,
    )
    notes = [" ".join(match.group("note").split()) for match in note_matches]
    actionable_notes = [
        note
        for note in notes
        if len(note) >= 60
        and all(placeholder not in note.lower() for placeholder in PLACEHOLDER_WORDS)
        and MIGRATION_GUIDANCE_PATTERN.search(note) is not None
    ]
    if not actionable_notes:
        raise ReleaseError(
            f"Rust release {release_version} needs an actionable migration note in "
            "its CHANGELOG entry using '- **Migration:** Breaking: ...'; describe "
            "the removed behavior and the concrete replacement"
        )


def write_github_output(path: Path | None, release: ReleaseInfo | None) -> None:
    if path is None:
        return
    lines = ["is-release=false"]
    if release is not None:
        lines = [
            "is-release=true",
            f"release-version={release.release_version}",
            f"semver-packages={','.join(SEMVER_LIBRARY_PACKAGES)}",
        ]
    try:
        with path.open("a", encoding="utf-8") as output:
            output.write("\n".join(lines) + "\n")
    except OSError as error:
        raise ReleaseError(f"cannot write GitHub output {path}: {error}") from error


def main() -> int:
    args = parse_args()
    repo_root = args.repo_root.resolve()
    try:
        release = detect_release(repo_root, args.base_rev)
        write_github_output(args.github_output, release)
        if release is None:
            if args.require_migration_notes:
                raise ReleaseError(
                    "migration-note validation requires a Rust release PR"
                )
            print(
                "no publishable Rust package version changed; SemVer check not required"
            )
            return 0
        if args.require_migration_notes:
            validate_migration_notes(repo_root, release.release_version)
            print(f"migration note covers Rust release {release.release_version}")
        else:
            print(
                f"Rust release detected: {release.base_version} -> "
                f"{release.release_version}"
            )
        return 0
    except ReleaseError as error:
        print(f"Rust release contract failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
