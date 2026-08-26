#!/usr/bin/env python3
"""Validate source and built artifacts against the package license policy."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tarfile
import tomllib
import zipfile
from email.parser import Parser
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = REPO_ROOT / "license-policy.toml"


class PolicyError(ValueError):
    """Raised when source or an artifact violates the license policy."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--source", action="store_true", help="validate source metadata")
    modes.add_argument("--rust", nargs="+", type=Path, metavar="CRATE")
    modes.add_argument("--python", nargs="+", type=Path, metavar="DIST")
    modes.add_argument("--npm", type=Path, metavar="PACKAGE_DIR")
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PolicyError(message)


def load_toml(path: Path) -> dict[str, Any]:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise PolicyError(f"cannot read TOML {path}: {error}") from error


def repository_path(value: str) -> Path:
    relative = Path(value)
    require(
        not relative.is_absolute() and ".." not in relative.parts,
        f"unsafe repository path in policy: {value}",
    )
    return REPO_ROOT / relative


def require_string(policy: dict[str, Any], key: str) -> str:
    value = policy.get(key)
    require(isinstance(value, str) and bool(value), f"policy {key} must be a string")
    return value


def require_string_list(policy: dict[str, Any], key: str) -> list[str]:
    value = policy.get(key)
    require(
        isinstance(value, list)
        and bool(value)
        and all(isinstance(item, str) and item for item in value),
        f"policy {key} must be a non-empty string list",
    )
    require(len(value) == len(set(value)), f"policy {key} contains duplicates")
    return value


def load_policy() -> dict[str, Any]:
    policy = load_toml(POLICY_PATH)
    require(policy.get("schema_version") == 1, "policy schema_version must be 1")
    for key in (
        "spdx_expression",
        "license_file",
        "license_sha256",
        "repository",
        "source_version",
        "current_since",
        "historical_dual_license_through",
    ):
        require_string(policy, key)
    for key in (
        "workspace_manifests",
        "package_license_files",
        "public_readmes",
        "rust_packages",
    ):
        require_string_list(policy, key)
    for key in ("python", "npm"):
        require(isinstance(policy.get(key), dict), f"policy [{key}] table is required")
    return policy


def canonical_license(policy: dict[str, Any]) -> bytes:
    path = repository_path(policy["license_file"])
    try:
        content = path.read_bytes()
    except OSError as error:
        raise PolicyError(f"cannot read canonical license {path}: {error}") from error
    digest = hashlib.sha256(content).hexdigest()
    require(
        digest == policy["license_sha256"],
        f"canonical license SHA-256 {digest} != policy {policy['license_sha256']}",
    )
    return content


def check_source(policy: dict[str, Any]) -> None:
    spdx = policy["spdx_expression"]
    repository = policy["repository"]
    license_content = canonical_license(policy)

    require(not (REPO_ROOT / "LICENSE-MIT").exists(), "stale LICENSE-MIT exists")
    require(
        not (REPO_ROOT / "LICENSE-APACHE").exists(),
        "stale LICENSE-APACHE exists; LICENSE is canonical",
    )
    for relative in policy["package_license_files"]:
        path = repository_path(relative)
        require(path.is_file(), f"package license is missing: {relative}")
        require(
            path.read_bytes() == license_content,
            f"package license differs from LICENSE: {relative}",
        )

    workspace = load_toml(REPO_ROOT / "Cargo.toml")
    workspace_package = workspace.get("workspace", {}).get("package", {})
    require(workspace_package.get("license") == spdx, "workspace SPDX mismatch")
    require(
        workspace_package.get("repository") == repository,
        "workspace repository mismatch",
    )

    actual_manifests = [
        f"{member}/Cargo.toml"
        for member in workspace.get("workspace", {}).get("members", [])
    ]
    require(
        actual_manifests == policy["workspace_manifests"],
        "policy workspace_manifests does not match Cargo workspace members",
    )

    published_packages: list[str] = []
    for relative in policy["workspace_manifests"]:
        manifest = load_toml(repository_path(relative))
        package = manifest.get("package", {})
        require(
            package.get("license") == {"workspace": True},
            f"{relative} must inherit workspace license metadata",
        )
        require(
            package.get("repository") == {"workspace": True},
            f"{relative} must inherit workspace repository metadata",
        )
        require(
            "license-file" not in package,
            f"{relative} must not combine license and license-file metadata",
        )
        require(
            package.get("version") == policy["source_version"],
            f"{relative} version does not match policy source_version",
        )
        if package.get("publish") is not False:
            name = package.get("name")
            require(isinstance(name, str), f"{relative} has no package name")
            published_packages.append(name)
    require(
        published_packages == policy["rust_packages"],
        f"published Rust packages {published_packages} != policy {policy['rust_packages']}",
    )

    python_policy = policy["python"]
    pyproject = load_toml(repository_path(python_policy["manifest"]))
    project = pyproject.get("project", {})
    require(
        project.get("name") == python_policy["distribution"], "Python name mismatch"
    )
    require(project.get("license") == spdx, "Python SPDX metadata mismatch")
    require(
        project.get("license-files") == ["LICENSE"],
        "Python project.license-files must contain LICENSE",
    )
    require(
        "License :: OSI Approved :: Apache Software License"
        in project.get("classifiers", []),
        "Python Apache classifier is missing",
    )
    require(
        repository_path(python_policy["license_file"]).read_bytes() == license_content,
        "Python package license differs from LICENSE",
    )

    npm_policy = policy["npm"]
    npm_manifest = load_toml(repository_path(npm_policy["manifest"]))
    require(
        npm_manifest.get("package", {}).get("name") == npm_policy["package"],
        "npm package name mismatch",
    )
    require(
        repository_path(npm_policy["license_file"]).read_bytes() == license_content,
        "npm package license differs from LICENSE",
    )

    expected_sentence = (
        "Licensed under the [Apache License, Version 2.0](../../LICENSE)."
    )
    for relative in policy["public_readmes"]:
        text = repository_path(relative).read_text(encoding="utf-8")
        require(
            expected_sentence in text, f"{relative} lacks current license statement"
        )
        require(
            "MIT OR Apache-2.0" not in text and "Dual-licensed" not in text,
            f"{relative} contains a stale dual-license statement",
        )

    root_readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
    require(
        "[license policy](docs/license.md)" in root_readme,
        "README must link to docs/license.md",
    )
    policy_doc = (REPO_ROOT / "docs" / "license.md").read_text(encoding="utf-8")
    for statement in (
        policy["spdx_expression"],
        policy["current_since"],
        policy["historical_dual_license_through"],
        "Third-party",
    ):
        require(statement in policy_doc, f"docs/license.md lacks {statement!r}")


def tar_member_bytes(archive: tarfile.TarFile, name: str) -> bytes:
    member = archive.getmember(name)
    extracted = archive.extractfile(member)
    require(extracted is not None, f"cannot read {name}")
    return extracted.read()


def check_rust(policy: dict[str, Any], paths: list[Path]) -> None:
    expected = set(policy["rust_packages"])
    found: set[str] = set()
    license_content = canonical_license(policy)
    for path in paths:
        require(path.is_file(), f"Rust artifact does not exist: {path}")
        with tarfile.open(path, "r:gz") as archive:
            cargo_tomls = [
                name
                for name in archive.getnames()
                if name.count("/") == 1 and name.endswith("/Cargo.toml")
            ]
            require(len(cargo_tomls) == 1, f"{path} has ambiguous Cargo.toml")
            manifest = tomllib.loads(
                tar_member_bytes(archive, cargo_tomls[0]).decode("utf-8")
            )
            package = manifest.get("package", {})
            name = package.get("name")
            version = package.get("version")
            require(name in expected, f"{path} has unexpected package {name!r}")
            require(name not in found, f"duplicate Rust artifact for {name}")
            require(version == policy["source_version"], f"{name} version mismatch")
            require(
                package.get("license") == policy["spdx_expression"],
                f"{name} SPDX metadata mismatch",
            )
            require(
                package.get("repository") == policy["repository"],
                f"{name} repository metadata mismatch",
            )
            root = cargo_tomls[0].split("/", 1)[0]
            require(
                tar_member_bytes(archive, f"{root}/LICENSE") == license_content,
                f"{name} LICENSE differs from canonical text",
            )
            found.add(name)
    require(found == expected, f"Rust artifacts {sorted(found)} != {sorted(expected)}")


def parsed_metadata(content: bytes, context: str) -> Any:
    try:
        return Parser().parsestr(content.decode("utf-8"))
    except UnicodeDecodeError as error:
        raise PolicyError(f"{context} metadata is not UTF-8: {error}") from error


def check_python_metadata(policy: dict[str, Any], metadata: Any, context: str) -> None:
    python_policy = policy["python"]
    require(
        metadata.get("Name") == python_policy["distribution"],
        f"{context} name mismatch",
    )
    require(
        metadata.get("Version") == policy["source_version"],
        f"{context} version mismatch",
    )
    expression = metadata.get("License-Expression") or metadata.get("License")
    require(
        expression == policy["spdx_expression"], f"{context} SPDX metadata mismatch"
    )


def check_python(policy: dict[str, Any], paths: list[Path]) -> None:
    wheels = [path for path in paths if path.name.endswith(".whl")]
    sdists = [path for path in paths if path.name.endswith(".tar.gz")]
    require(bool(wheels), "no Python wheel supplied")
    require(bool(sdists), "no Python source distribution supplied")
    require(len(wheels) + len(sdists) == len(paths), "unknown Python artifact supplied")
    license_content = canonical_license(policy)

    for path in wheels:
        require(path.is_file(), f"Python wheel does not exist: {path}")
        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            metadata_names = [
                name for name in names if name.endswith(".dist-info/METADATA")
            ]
            license_names = [
                name for name in names if name.endswith(".dist-info/licenses/LICENSE")
            ]
            require(len(metadata_names) == 1, f"{path} has ambiguous METADATA")
            require(
                len(license_names) == 1, f"{path} must contain one packaged LICENSE"
            )
            check_python_metadata(
                policy,
                parsed_metadata(archive.read(metadata_names[0]), str(path)),
                str(path),
            )
            require(
                archive.read(license_names[0]) == license_content,
                f"{path} LICENSE differs from canonical text",
            )

    for path in sdists:
        require(path.is_file(), f"Python sdist does not exist: {path}")
        with tarfile.open(path, "r:gz") as archive:
            roots = {
                name.split("/", 1)[0] for name in archive.getnames() if "/" in name
            }
            require(len(roots) == 1, f"{path} has ambiguous archive root")
            root = roots.pop()
            check_python_metadata(
                policy,
                parsed_metadata(
                    tar_member_bytes(archive, f"{root}/PKG-INFO"), str(path)
                ),
                str(path),
            )
            require(
                tar_member_bytes(archive, f"{root}/LICENSE") == license_content,
                f"{path} LICENSE differs from canonical text",
            )


def check_npm(policy: dict[str, Any], directory: Path) -> None:
    require(directory.is_dir(), f"npm package directory does not exist: {directory}")
    try:
        package = json.loads((directory / "package.json").read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PolicyError(f"cannot read npm package.json: {error}") from error
    npm_policy = policy["npm"]
    require(package.get("name") == npm_policy["package"], "npm package name mismatch")
    require(package.get("version") == policy["source_version"], "npm version mismatch")
    require(package.get("license") == policy["spdx_expression"], "npm SPDX mismatch")
    require(
        (directory / "LICENSE").read_bytes() == canonical_license(policy),
        "npm LICENSE differs from canonical text",
    )


def main() -> int:
    args = parse_args()
    try:
        policy = load_policy()
        if args.source:
            check_source(policy)
            family = "source"
        elif args.rust:
            check_rust(policy, args.rust)
            family = "Rust"
        elif args.python:
            check_python(policy, args.python)
            family = "Python"
        else:
            check_npm(policy, args.npm)
            family = "npm"
    except (
        OSError,
        PolicyError,
        KeyError,
        tarfile.TarError,
        zipfile.BadZipFile,
    ) as error:
        print(f"package license check failed: {error}", file=sys.stderr)
        return 1
    print(f"{family} package license policy verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
