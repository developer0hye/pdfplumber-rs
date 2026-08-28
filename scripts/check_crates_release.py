#!/usr/bin/env python3
"""Build and dry-run every crates.io package from one exact source commit."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tarfile
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SHA1_PATTERN = re.compile(r"^[0-9a-f]{40}$")


class CratesReleaseError(RuntimeError):
    """Raised when the release-package boundary cannot be proven."""


@dataclass(frozen=True)
class WorkspacePackage:
    """A crates.io package and its local publishable dependencies."""

    name: str
    version: str
    manifest_path: Path
    dependencies: frozenset[str]

    @property
    def root(self) -> Path:
        return self.manifest_path.parent


@dataclass(frozen=True)
class Workspace:
    """Cargo metadata required by the package gate."""

    root: Path
    target_directory: Path
    packages: tuple[WorkspacePackage, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="workspace root (defaults to the checker repository)",
    )
    source = parser.add_mutually_exclusive_group()
    source.add_argument(
        "--expected-commit",
        help="full commit SHA that must equal the checked-out source",
    )
    source.add_argument(
        "--release-tag",
        help="release tag whose commit and version must equal the checkout",
    )
    parser.add_argument(
        "--package-only",
        action="store_true",
        help="run verified cargo package commands without publish dry runs",
    )
    parser.add_argument(
        "--list-packages",
        action="store_true",
        help="print the discovered crates.io package order as JSON and exit",
    )
    return parser.parse_args()


def run_capture(command: Sequence[str], cwd: Path) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CratesReleaseError(f"{' '.join(command)} failed: {detail}")
    return result.stdout.strip()


def run_live(command: Sequence[str], cwd: Path) -> None:
    print(f"+ {' '.join(command)}", flush=True)
    result = subprocess.run(command, cwd=cwd, check=False)
    if result.returncode != 0:
        raise CratesReleaseError(
            f"{' '.join(command)} failed with exit code {result.returncode}"
        )


def load_json(value: str, context: str) -> dict[str, Any]:
    try:
        document = json.loads(value)
    except json.JSONDecodeError as error:
        raise CratesReleaseError(f"invalid JSON from {context}: {error}") from error
    if not isinstance(document, dict):
        raise CratesReleaseError(f"{context} did not return a JSON object")
    return document


def is_crates_io_package(package: dict[str, Any]) -> bool:
    publish = package.get("publish")
    return publish is None or (
        isinstance(publish, list) and "crates-io" in publish
    )


def topological_packages(
    packages: dict[str, WorkspacePackage],
) -> tuple[WorkspacePackage, ...]:
    remaining = dict(packages)
    ordered: list[WorkspacePackage] = []
    completed: set[str] = set()
    while remaining:
        ready = sorted(
            [
                package
                for package in remaining.values()
                if package.dependencies <= completed
            ],
            key=lambda package: package.name,
        )
        if not ready:
            unresolved = ", ".join(sorted(remaining))
            raise CratesReleaseError(
                f"publishable workspace dependency cycle: {unresolved}"
            )
        for package in ready:
            ordered.append(package)
            completed.add(package.name)
            del remaining[package.name]
    return tuple(ordered)


def discover_workspace(repo_root: Path) -> Workspace:
    root = repo_root.resolve()
    metadata = load_json(
        run_capture(
            ("cargo", "metadata", "--format-version", "1", "--no-deps"),
            root,
        ),
        "cargo metadata",
    )
    raw_packages = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    if not isinstance(raw_packages, list) or not isinstance(workspace_members, list):
        raise CratesReleaseError("cargo metadata omitted workspace packages")

    raw_by_id = {
        package.get("id"): package
        for package in raw_packages
        if isinstance(package, dict) and isinstance(package.get("id"), str)
    }
    publishable_raw = [
        raw_by_id[package_id]
        for package_id in workspace_members
        if package_id in raw_by_id and is_crates_io_package(raw_by_id[package_id])
    ]
    publishable_names = {
        package.get("name")
        for package in publishable_raw
        if isinstance(package.get("name"), str)
    }
    packages: dict[str, WorkspacePackage] = {}
    for package in publishable_raw:
        name = package.get("name")
        version = package.get("version")
        manifest_path = package.get("manifest_path")
        dependencies = package.get("dependencies")
        if (
            not isinstance(name, str)
            or not isinstance(version, str)
            or not isinstance(manifest_path, str)
            or not isinstance(dependencies, list)
        ):
            raise CratesReleaseError("cargo metadata contains an invalid package")
        local_dependencies = frozenset(
            dependency["name"]
            for dependency in dependencies
            if isinstance(dependency, dict)
            and dependency.get("kind") != "dev"
            and dependency.get("name") in publishable_names
            and dependency.get("path") is not None
        )
        packages[name] = WorkspacePackage(
            name=name,
            version=version,
            manifest_path=Path(manifest_path).resolve(),
            dependencies=local_dependencies,
        )

    if not packages:
        raise CratesReleaseError("workspace has no crates.io packages")
    target_directory = metadata.get("target_directory")
    if not isinstance(target_directory, str):
        raise CratesReleaseError("cargo metadata omitted target_directory")
    return Workspace(
        root=root,
        target_directory=Path(target_directory).resolve(),
        packages=topological_packages(packages),
    )


def exact_source_commit(
    workspace: Workspace,
    *,
    expected_commit: str | None,
    release_tag: str | None,
) -> str:
    current_commit = run_capture(
        ("git", "rev-parse", "HEAD^{commit}"), workspace.root
    ).lower()
    if not SHA1_PATTERN.fullmatch(current_commit):
        raise CratesReleaseError(f"invalid checked-out commit: {current_commit}")

    if release_tag is not None:
        versions = {package.version for package in workspace.packages}
        if len(versions) != 1:
            raise CratesReleaseError(
                "crates.io package versions must be synchronized for a release tag"
            )
        version = next(iter(versions))
        if release_tag != f"v{version}":
            raise CratesReleaseError(
                f"release tag {release_tag} does not match package version {version}"
            )
        expected_commit = run_capture(
            (
                "git",
                "rev-parse",
                "--verify",
                f"refs/tags/{release_tag}^{{commit}}",
            ),
            workspace.root,
        ).lower()

    if expected_commit is None:
        raise CratesReleaseError(
            "an exact source commit is required via --expected-commit or --release-tag"
        )
    expected_commit = expected_commit.lower()
    if not SHA1_PATTERN.fullmatch(expected_commit):
        raise CratesReleaseError(
            f"exact source commit must be a full 40-character SHA-1: {expected_commit}"
        )
    if current_commit != expected_commit:
        raise CratesReleaseError(
            "exact source commit mismatch: "
            f"expected {expected_commit}, checked out {current_commit}"
        )

    dirty = run_capture(
        ("git", "status", "--porcelain", "--untracked-files=all"),
        workspace.root,
    )
    if dirty:
        raise CratesReleaseError(
            "exact source commit worktree is dirty; commit or remove every change first"
        )
    return current_commit


def transitive_dependencies(
    package: WorkspacePackage,
    packages: dict[str, WorkspacePackage],
) -> tuple[WorkspacePackage, ...]:
    names: set[str] = set()
    pending = list(package.dependencies)
    while pending:
        dependency_name = pending.pop()
        if dependency_name in names:
            continue
        names.add(dependency_name)
        pending.extend(packages[dependency_name].dependencies)
    return tuple(
        candidate for candidate in packages.values() if candidate.name in names
    )


def candidate_patch_arguments(
    package: WorkspacePackage,
    packages: dict[str, WorkspacePackage],
) -> list[str]:
    arguments: list[str] = []
    for dependency in transitive_dependencies(package, packages):
        quoted_path = json.dumps(str(dependency.root))
        arguments.extend(
            (
                "--config",
                f"patch.crates-io.{dependency.name}.path={quoted_path}",
            )
        )
    return arguments


def archive_path(workspace: Workspace, package: WorkspacePackage) -> Path:
    return (
        workspace.target_directory
        / "package"
        / f"{package.name}-{package.version}.crate"
    )


def verify_archive(
    archive: Path,
    package: WorkspacePackage,
    source_commit: str,
) -> None:
    if not archive.is_file():
        raise CratesReleaseError(f"Cargo did not create {archive}")
    vcs_member = f"{package.name}-{package.version}/.cargo_vcs_info.json"
    try:
        with tarfile.open(archive, mode="r:gz") as package_archive:
            member = package_archive.extractfile(vcs_member)
            if member is None:
                raise CratesReleaseError(f"{archive} omits {vcs_member}")
            vcs_info = load_json(member.read().decode("utf-8"), vcs_member)
    except (OSError, tarfile.TarError, UnicodeError) as error:
        raise CratesReleaseError(f"cannot inspect {archive}: {error}") from error

    git = vcs_info.get("git")
    if not isinstance(git, dict):
        raise CratesReleaseError(f"{archive} has no Git provenance")
    dirty = git.get("dirty", False)
    if (
        git.get("sha1") != source_commit
        or not isinstance(dirty, bool)
        or dirty
    ):
        raise CratesReleaseError(
            f"{archive} is not bound to clean commit {source_commit}: {git}"
        )
    print(f"verified {archive.name} at {source_commit}", flush=True)


def run_gate(
    workspace: Workspace,
    source_commit: str,
    *,
    package_only: bool,
) -> None:
    packages = {package.name: package for package in workspace.packages}
    for package in workspace.packages:
        patch_arguments = candidate_patch_arguments(package, packages)
        archive = archive_path(workspace, package)
        archive.unlink(missing_ok=True)
        run_live(
            (
                "cargo",
                "package",
                "--package",
                package.name,
                *patch_arguments,
            ),
            workspace.root,
        )
        verify_archive(archive, package, source_commit)

        if package_only:
            continue
        archive.unlink(missing_ok=True)
        run_live(
            (
                "cargo",
                "publish",
                "--dry-run",
                "--package",
                package.name,
                *patch_arguments,
            ),
            workspace.root,
        )
        verify_archive(archive, package, source_commit)

    actions = "cargo package"
    if not package_only:
        actions += " and cargo publish --dry-run"
    print(
        f"verified {len(workspace.packages)} crates.io packages with {actions} "
        f"from exact commit {source_commit}"
    )


def main() -> int:
    arguments = parse_args()
    try:
        workspace = discover_workspace(arguments.repo_root)
        if arguments.list_packages:
            print(json.dumps([package.name for package in workspace.packages]))
            return 0
        source_commit = exact_source_commit(
            workspace,
            expected_commit=arguments.expected_commit,
            release_tag=arguments.release_tag,
        )
        run_gate(
            workspace,
            source_commit,
            package_only=arguments.package_only,
        )
    except CratesReleaseError as error:
        print(f"crates release gate failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
