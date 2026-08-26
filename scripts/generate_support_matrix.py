#!/usr/bin/env python3
"""Generate or verify the public support matrix from validated source data."""

from __future__ import annotations

import argparse
import difflib
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_PATH = REPO_ROOT / "support-matrix.toml"
OUTPUT_PATH = REPO_ROOT / "docs" / "support.md"
SURFACE_IDS = ("rust", "python", "cli", "wasm")
MATURITIES = {"experimental", "alpha", "beta", "stable"}
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


class MatrixError(ValueError):
    """Raised when the support-matrix source violates its contract."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if docs/support.md is missing or stale",
    )
    return parser.parse_args()


def require_string(mapping: dict[str, Any], key: str, context: str) -> str:
    value = mapping.get(key)
    if not isinstance(value, str) or not value.strip():
        raise MatrixError(f"{context}.{key} must be a non-empty string")
    return value


def require_string_list(
    mapping: dict[str, Any], key: str, context: str
) -> list[str]:
    value = mapping.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item.strip() for item in value)
    ):
        raise MatrixError(f"{context}.{key} must be a non-empty string list")
    if len(value) != len(set(value)):
        raise MatrixError(f"{context}.{key} contains duplicates")
    return value


def repository_path(value: str, context: str) -> Path:
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise MatrixError(f"{context} must be a safe repository-relative path")
    path = REPO_ROOT / relative
    if not path.is_file():
        raise MatrixError(f"{context} does not exist: {value}")
    return path


def load_source(path: Path = SOURCE_PATH) -> dict[str, Any]:
    try:
        source: dict[str, Any] = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise MatrixError(f"cannot read {path.relative_to(REPO_ROOT)}: {error}") from error

    if source.get("schema_version") != 1:
        raise MatrixError("schema_version must be 1")
    observed_at = require_string(source, "observed_at", "matrix")
    if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", observed_at):
        raise MatrixError("matrix.observed_at must use YYYY-MM-DD")
    release_version = require_string(source, "release_version", "matrix")
    if not VERSION_PATTERN.fullmatch(release_version):
        raise MatrixError("matrix.release_version must be a semantic version")
    rust_version = require_string(source, "rust_version", "matrix")

    workspace = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    actual_rust_version = workspace["workspace"]["package"]["rust-version"]
    if rust_version != actual_rust_version:
        raise MatrixError(
            f"matrix rust_version {rust_version} != workspace {actual_rust_version}"
        )

    surfaces = source.get("surfaces")
    if not isinstance(surfaces, list):
        raise MatrixError("surfaces must be an array of tables")
    ids = tuple(surface.get("id") for surface in surfaces if isinstance(surface, dict))
    if ids != SURFACE_IDS:
        raise MatrixError(f"surface ids/order must be {SURFACE_IDS}, got {ids}")

    for surface in surfaces:
        surface_id = require_string(surface, "id", "surface")
        context = f"surface[{surface_id}]"
        for key in (
            "name",
            "package",
            "registry",
            "registry_url",
            "manifest",
            "manifest_package",
            "source_version",
            "registry_version",
        ):
            require_string(surface, key, context)

        maturity = require_string(surface, "maturity", context)
        if maturity not in MATURITIES:
            raise MatrixError(f"{context}.maturity must be one of {sorted(MATURITIES)}")

        source_version = surface["source_version"]
        registry_version = surface["registry_version"]
        if not VERSION_PATTERN.fullmatch(source_version):
            raise MatrixError(f"{context}.source_version must be a semantic version")
        if not VERSION_PATTERN.fullmatch(registry_version):
            raise MatrixError(f"{context}.registry_version must be a semantic version")
        if source_version != release_version:
            raise MatrixError(
                f"{context}.source_version {source_version} != release {release_version}"
            )

        manifest_path = repository_path(surface["manifest"], f"{context}.manifest")
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = manifest.get("package", {})
        if package.get("name") != surface["manifest_package"]:
            raise MatrixError(
                f"{context}.manifest_package {surface['manifest_package']} "
                f"!= manifest {package.get('name')}"
            )
        if package.get("version") != source_version:
            raise MatrixError(
                f"{context}.source_version {source_version} "
                f"!= manifest {package.get('version')}"
            )

        for key in (
            "ci_verified_platforms",
            "release_configured_targets",
            "features",
            "known_limitations",
            "evidence",
        ):
            values = require_string_list(surface, key, context)
            if key == "evidence":
                for index, value in enumerate(values):
                    repository_path(value, f"{context}.evidence[{index}]")

    return source


def escape_cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def render_list(values: list[str]) -> list[str]:
    return [f"- {value}" for value in values]


def render(source: dict[str, Any]) -> str:
    lines = [
        "# Support matrix",
        "",
        "<!-- Generated by scripts/generate_support_matrix.py from support-matrix.toml. Do not edit this file directly. -->",
        "",
        f"This snapshot describes repository release `{source['release_version']}` and registry state observed on {source['observed_at']}. It distinguishes required CI evidence from targets that release automation is merely configured to build.",
        "",
        "Maturity is assigned per surface using the [surface maturity contract](../PRD.md#06-surface-maturity-contract). `Experimental` and `alpha` surfaces may change; neither label is a production-readiness promise.",
        "",
        "## Surface summary",
        "",
        "| Surface | Maturity | Package | Source version | Observed registry version |",
        "|---|---|---|---|---|",
    ]

    for surface in source["surfaces"]:
        package = (
            f"[`{surface['package']}`]({surface['registry_url']}) "
            f"({surface['registry']})"
        )
        lines.append(
            "| "
            + " | ".join(
                (
                    escape_cell(surface["name"]),
                    surface["maturity"].title(),
                    escape_cell(package),
                    f"`{surface['source_version']}`",
                    f"`{surface['registry_version']}` as of {source['observed_at']}",
                )
            )
            + " |"
        )

    lines.extend(
        (
            "",
            "A release-configured target is not considered supported or CI-verified until an installed artifact is exercised on that target. Registry versions are dated observations and may change independently of this repository snapshot.",
            "",
            "## Platform evidence",
        )
    )

    for surface in source["surfaces"]:
        lines.extend(("", f"### {surface['name']}", "", "**CI-verified platforms**", ""))
        lines.extend(render_list(surface["ci_verified_platforms"]))
        lines.extend(("", "**Release-configured targets**", ""))
        lines.extend(render_list(surface["release_configured_targets"]))

    lines.extend(("", "## Features and known limitations"))
    for surface in source["surfaces"]:
        lines.extend(("", f"### {surface['name']}", "", "**Features**", ""))
        lines.extend(render_list(surface["features"]))
        lines.extend(("", "**Known limitations**", ""))
        lines.extend(render_list(surface["known_limitations"]))
        lines.extend(("", "**Evidence**", ""))
        for evidence in surface["evidence"]:
            lines.append(f"- [`{evidence}`](../{evidence})")

    lines.extend(
        (
            "",
            "## Updating the matrix",
            "",
            "Edit `support-matrix.toml`, refresh dated registry observations from the official registries, then run:",
            "",
            "```console",
            "python3 scripts/generate_support_matrix.py",
            "python3 scripts/generate_support_matrix.py --check",
            "```",
            "",
            "The generator validates all four required surfaces, their source manifest names and versions, the workspace Minimum Supported Rust Version, evidence paths, maturity values, and deterministic output. Continuous Integration rejects stale generated content.",
            "",
        )
    )
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    try:
        generated = render(load_source())
    except MatrixError as error:
        print(f"support matrix validation failed: {error}", file=sys.stderr)
        return 1

    if args.check:
        if not OUTPUT_PATH.is_file():
            print(f"support matrix is missing: {OUTPUT_PATH.relative_to(REPO_ROOT)}")
            return 1
        committed = OUTPUT_PATH.read_text(encoding="utf-8")
        if committed == generated:
            print(f"support matrix is current: {OUTPUT_PATH.relative_to(REPO_ROOT)}")
            return 0
        print(f"support matrix is stale: {OUTPUT_PATH.relative_to(REPO_ROOT)}")
        diff = difflib.unified_diff(
            committed.splitlines(),
            generated.splitlines(),
            fromfile="committed",
            tofile="generated",
            lineterm="",
        )
        for line in list(diff)[:200]:
            print(line)
        return 1

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(generated, encoding="utf-8")
    print(f"wrote support matrix: {OUTPUT_PATH.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
