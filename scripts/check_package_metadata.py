#!/usr/bin/env python3
"""Validate public package, documentation, artifact, and release metadata."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tarfile
import zipfile
from email.parser import Parser
from pathlib import Path
from typing import Any

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
LICENSE_POLICY_PATH = REPO_ROOT / "license-policy.toml"
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"
SURFACE_IDS = ("rust", "python", "cli", "wasm")
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
DESCRIPTION_MARKER = "evidence-driven pdf extraction"
FORBIDDEN_DESCRIPTION_CLAIMS = (
    "100% compatible",
    "complete drop-in",
    "complete replacement",
    "fully compatible",
    "full drop-in",
)
PYTHON_MATURITY_CLASSIFIERS = {
    "experimental": "Development Status :: 2 - Pre-Alpha",
    "alpha": "Development Status :: 3 - Alpha",
    "beta": "Development Status :: 4 - Beta",
    "stable": "Development Status :: 5 - Production/Stable",
}


class MetadataError(ValueError):
    """Raised when source or built package metadata disagrees."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--source", action="store_true")
    modes.add_argument("--rust", nargs="+", type=Path, metavar="CRATE")
    modes.add_argument("--python", nargs="+", type=Path, metavar="DIST")
    modes.add_argument("--npm", type=Path, metavar="PACKAGE_DIR")
    modes.add_argument("--release-tag", metavar="TAG")
    parser.add_argument("--github-output", type=Path, metavar="PATH")
    return parser.parse_args()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MetadataError(message)


def load_toml(path: Path) -> dict[str, Any]:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise MetadataError(f"cannot read TOML {path}: {error}") from error


def repository_path(value: str, context: str) -> Path:
    relative = Path(value)
    require(
        not relative.is_absolute() and ".." not in relative.parts,
        f"{context} must be a safe repository-relative path: {value}",
    )
    return REPO_ROOT / relative


def require_string(mapping: dict[str, Any], key: str, context: str) -> str:
    value = mapping.get(key)
    require(
        isinstance(value, str) and bool(value.strip()),
        f"{context}.{key} must be a non-empty string",
    )
    return value


def require_string_list(
    mapping: dict[str, Any],
    key: str,
    context: str,
    *,
    minimum: int = 1,
) -> list[str]:
    value = mapping.get(key)
    require(
        isinstance(value, list)
        and len(value) >= minimum
        and all(isinstance(item, str) and bool(item.strip()) for item in value),
        f"{context}.{key} must contain at least {minimum} non-empty strings",
    )
    require(
        len(value) == len(set(value)),
        f"{context}.{key} must not contain duplicate strings",
    )
    return value


def validate_public_description(
    description: str,
    maturity: str,
    context: str,
    *,
    maximum_length: int = 200,
) -> None:
    lowered = description.lower()
    require(
        DESCRIPTION_MARKER in lowered,
        f"{context} must use the evidence-driven PDF extraction position",
    )
    require(maturity in lowered, f"{context} must display {maturity} maturity")
    require(
        len(description) <= maximum_length,
        f"{context} exceeds {maximum_length} characters",
    )
    require(
        "`" not in description and "\n" not in description,
        f"{context} must be plain single-line metadata",
    )
    for phrase in FORBIDDEN_DESCRIPTION_CLAIMS:
        require(phrase not in lowered, f"{context} overclaims {phrase!r}")


def load_matrix() -> dict[str, Any]:
    matrix = load_toml(MATRIX_PATH)
    require(matrix.get("schema_version") == 1, "matrix schema_version must be 1")
    observed_at = require_string(matrix, "observed_at", "matrix")
    require(
        bool(re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", observed_at)),
        "matrix observed_at must use YYYY-MM-DD",
    )
    version = require_string(matrix, "release_version", "matrix")
    require(bool(VERSION_PATTERN.fullmatch(version)), "invalid matrix release_version")
    for key in (
        "license",
        "repository",
        "release_notes",
        "positioning",
        "github_description",
    ):
        require_string(matrix, key, "matrix")
    require(
        matrix["github_description"] == matrix["positioning"],
        "matrix GitHub description must equal the canonical positioning",
    )
    validate_public_description(
        matrix["positioning"],
        "alpha",
        "matrix.positioning",
        maximum_length=160,
    )
    require(
        isinstance(matrix.get("github_prerelease"), bool),
        "matrix.github_prerelease must be a boolean",
    )
    require(
        matrix["release_notes"] == f"docs/releases/v{version}.md",
        "matrix release_notes must be versioned from release_version",
    )
    require_string_list(
        matrix,
        "release_upgrade_guidance",
        "matrix",
        minimum=3,
    )

    surfaces = matrix.get("surfaces")
    require(isinstance(surfaces, list), "matrix surfaces must be an array")
    require(
        tuple(surface.get("id") for surface in surfaces) == SURFACE_IDS,
        f"matrix surface ids/order must be {SURFACE_IDS}",
    )
    for surface in surfaces:
        surface_id = require_string(surface, "id", "surface")
        context = f"surface[{surface_id}]"
        for key in (
            "name",
            "maturity",
            "package",
            "readme",
            "manifest",
            "manifest_package",
            "source_version",
            "registry",
            "registry_url",
            "registry_description",
            "observed_registry_description",
            "registry_version",
        ):
            require_string(surface, key, context)
        require_string_list(surface, "ci_verified_platforms", context)
        require_string_list(surface, "known_limitations", context)
        validate_public_description(
            surface["registry_description"],
            surface["maturity"],
            f"{context}.registry_description",
        )
        require(
            surface["source_version"] == version,
            f"{context}.source_version disagrees with release_version",
        )
        if surface_id == "cli":
            require_string(surface, "executable", context)
        else:
            require_string(surface, "import_name", context)
        if surface_id == "python":
            require_string(surface, "native_module", context)
    expected_prerelease = any(surface["maturity"] != "stable" for surface in surfaces)
    require(
        matrix["github_prerelease"] == expected_prerelease,
        "matrix.github_prerelease disagrees with surface maturity",
    )
    return matrix


def surfaces_by_id(matrix: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {surface["id"]: surface for surface in matrix["surfaces"]}


def changelog_release(matrix: dict[str, Any]) -> tuple[str, str]:
    try:
        changelog = CHANGELOG_PATH.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise MetadataError(
            f"cannot read changelog {CHANGELOG_PATH}: {error}"
        ) from error

    version = re.escape(matrix["release_version"])
    match = re.search(
        rf"^## \[{version}\] - (?P<date>\d{{4}}-\d{{2}}-\d{{2}})\n"
        r"(?P<body>.*?)(?=^## |^\[Unreleased\]:|\Z)",
        changelog,
        re.MULTILINE | re.DOTALL,
    )
    require(
        match is not None, f"CHANGELOG.md has no release {matrix['release_version']}"
    )
    release_body = match.group("body").strip()
    require(bool(release_body), "changelog release entry must not be empty")
    return match.group("date"), release_body


def resolved_package_value(
    package: dict[str, Any],
    workspace_package: dict[str, Any],
    key: str,
) -> Any:
    value = package.get(key)
    if value == {"workspace": True}:
        return workspace_package.get(key)
    return value


def package_maturity(package: dict[str, Any], context: str) -> str:
    metadata = package.get("metadata", {})
    require(isinstance(metadata, dict), f"{context} package.metadata must be a table")
    project_metadata = metadata.get("pdfplumber-rs", {})
    require(
        isinstance(project_metadata, dict),
        f"{context} package.metadata.pdfplumber-rs must be a table",
    )
    return require_string(project_metadata, "maturity", f"{context}.metadata")


def render_release_notes(matrix: dict[str, Any]) -> str:
    surfaces = surfaces_by_id(matrix)
    identity_rows = []
    artifact_rows = []
    limitation_rows = []
    for surface_id in SURFACE_IDS:
        surface = surfaces[surface_id]
        interface = (
            surface["executable"] if surface_id == "cli" else surface["import_name"]
        )
        identity_rows.append(
            f"| {surface['name']} | `{surface['package']}` | "
            f"`{interface}` | {surface['maturity'].title()} |"
        )
        release_state = (
            "Published for this release"
            if surface["source_version"] == surface["registry_version"]
            else "Not published for this release"
        )
        artifact_rows.append(
            f"| {surface['name']} | [`{surface['package']}`]({surface['registry_url']}) "
            f"({surface['registry']}) | `{surface['source_version']}` | "
            f"`{surface['registry_version']}` | {release_state} | "
            f"{surface['ci_verified_platforms'][0]} |"
        )
        limitation_rows.extend(
            f"- **{surface['name']}:** {limitation}"
            for limitation in surface["known_limitations"]
        )
    release_date, release_changes = changelog_release(matrix)
    changelog_anchor = matrix["release_version"].replace(".", "") + "---" + release_date
    repository_files = f"{matrix['repository']}/blob/main"
    wasm = surfaces["wasm"]
    return (
        f"# v{matrix['release_version']} release notes\n\n"
        "<!-- Generated by scripts/generate_release_notes.py from "
        "support-matrix.toml and CHANGELOG.md. Do not edit directly. -->\n\n"
        f"- Version: `{matrix['release_version']}`\n"
        f"- License: `{matrix['license']}`\n"
        f"- Repository: {matrix['repository']}\n\n"
        "| Surface | Package | Import or executable | Maturity |\n"
        "|---|---|---|---|\n" + "\n".join(identity_rows) + "\n\n"
        "This GitHub release is a prerelease because its public surfaces are "
        "classified as alpha or experimental. "
        f"The WebAssembly source is version `{wasm['source_version']}`, but the "
        f"observed npm package remains at `{wasm['registry_version']}` because it "
        "was not published with this release.\n\n"
        "## Who should upgrade?\n\n"
        + "\n".join(f"- {guidance}" for guidance in matrix["release_upgrade_guidance"])
        + "\n\n"
        "## Behavior changes\n\n"
        f"The [v{matrix['release_version']} changelog entry]"
        f"({repository_files}/CHANGELOG.md#{changelog_anchor}) is the canonical "
        "change record.\n\n"
        f"{release_changes}\n\n"
        "## Known limitations\n\n" + "\n".join(limitation_rows) + "\n\n"
        "## Artifact matrix\n\n"
        f"Registry versions were observed on {matrix['observed_at']}. Required CI "
        "evidence is narrower than release-configured targets.\n\n"
        "| Surface | Registry artifact | Source version | Observed registry version | "
        "Release state | Required CI evidence |\n"
        "|---|---|---|---|---|---|\n" + "\n".join(artifact_rows) + "\n\n"
        "## Evidence\n\n"
        f"- [Changelog]({repository_files}/CHANGELOG.md#{changelog_anchor}) — curated "
        "user-visible changes and migration notes.\n"
        f"- [Support matrix]({repository_files}/docs/support.md) — dated registry, "
        "platform, maturity, feature, and limitation evidence.\n"
        f"- [Readiness snapshot]({repository_files}/docs/readiness/"
        f"v{matrix['release_version']}.md) — test-backed workflows that are ready to "
        "evaluate.\n"
        f"- [Evidence ledger]({repository_files}/PRD.md#13-evidence-ledger) — exact "
        "task, pull request, and gate evidence.\n"
        f"- [Continuous Integration gates]({repository_files}/.github/workflows/"
        "ci.yml) — required source and artifact checks.\n"
        f"- [Release workflow]({repository_files}/.github/workflows/release.yml) — "
        "tag validation and registry publication configuration.\n\n"
        "These curated sections are prepended to the automatically generated pull-"
        "request list by the release workflow.\n"
    )


def check_source(matrix: dict[str, Any]) -> None:
    surfaces = surfaces_by_id(matrix)
    policy = load_toml(LICENSE_POLICY_PATH)
    require(
        policy.get("spdx_expression") == matrix["license"],
        "license policy and support matrix SPDX expressions disagree",
    )
    require(
        policy.get("repository") == matrix["repository"],
        "license policy and support matrix repositories disagree",
    )
    require(
        policy.get("source_version") == matrix["release_version"],
        "license policy and support matrix versions disagree",
    )

    workspace = load_toml(REPO_ROOT / "Cargo.toml")
    workspace_package = workspace.get("workspace", {}).get("package", {})
    require(
        workspace_package.get("license") == matrix["license"],
        "workspace license disagrees with support matrix",
    )
    require(
        workspace_package.get("repository") == matrix["repository"],
        "workspace repository disagrees with support matrix",
    )

    surface_by_manifest = {
        surface["manifest"]: surface for surface in matrix["surfaces"]
    }
    members = workspace.get("workspace", {}).get("members", [])
    require(
        isinstance(members, list) and bool(members), "workspace members are missing"
    )
    for member in members:
        relative = f"{member}/Cargo.toml"
        manifest = load_toml(repository_path(relative, "workspace member"))
        package = manifest.get("package", {})
        require(isinstance(package, dict), f"{relative} package table is missing")
        require(
            package.get("version") == matrix["release_version"],
            f"{relative} version disagrees with release_version",
        )
        require(
            resolved_package_value(package, workspace_package, "license")
            == matrix["license"],
            f"{relative} license disagrees with support matrix",
        )
        require(
            resolved_package_value(package, workspace_package, "repository")
            == matrix["repository"],
            f"{relative} repository disagrees with support matrix",
        )
        surface = surface_by_manifest.get(relative, surfaces["rust"])
        maturity = package_maturity(package, relative)
        require(
            maturity == surface["maturity"],
            f"{relative} maturity {maturity} != {surface['maturity']}",
        )
        description = require_string(package, "description", f"{relative}.package")
        validate_public_description(
            description,
            surface["maturity"],
            f"{relative} description",
        )
        if relative in surface_by_manifest:
            require(
                package.get("name") == surface["manifest_package"],
                f"{relative} manifest package name disagrees with support matrix",
            )
            require(
                description == surface["registry_description"],
                f"{relative} description disagrees with support matrix",
            )

    rust = surfaces["rust"]
    rust_manifest = load_toml(repository_path(rust["manifest"], "Rust manifest"))
    rust_package = rust_manifest["package"]
    require(
        rust_package.get("exclude") == ["tests/fixtures/"],
        "Rust package must exclude the fixture corpus",
    )
    rust_import = rust_manifest.get("lib", {}).get(
        "name", rust_package["name"].replace("-", "_")
    )
    require(rust_import == rust["import_name"], "Rust import name mismatch")

    cli = surfaces["cli"]
    cli_manifest = load_toml(repository_path(cli["manifest"], "CLI manifest"))
    bins = cli_manifest.get("bin", [])
    require(
        [target.get("name") for target in bins] == [cli["executable"]],
        "CLI executable name mismatch",
    )

    python = surfaces["python"]
    pyproject_path = REPO_ROOT / "crates" / "pdfplumber-py" / "pyproject.toml"
    pyproject = load_toml(pyproject_path)
    project = pyproject.get("project", {})
    require(project.get("name") == python["package"], "Python distribution mismatch")
    require(project.get("license") == matrix["license"], "Python license mismatch")
    require(
        project.get("dynamic") == ["version"],
        "Python version must remain sourced from its Cargo manifest",
    )
    require(
        pyproject.get("tool", {}).get("maturin", {}).get("module-name")
        == python["native_module"],
        "Python native module mismatch",
    )
    require(
        project.get("urls", {}).get("Repository") == matrix["repository"],
        "Python repository URL mismatch",
    )
    classifier = PYTHON_MATURITY_CLASSIFIERS[python["maturity"]]
    require(
        classifier in project.get("classifiers", []),
        f"Python maturity classifier is missing: {classifier}",
    )
    require(
        python["import_name"] == python["native_module"].split(".", 1)[0],
        "Python import and native module roots disagree",
    )
    require(
        require_string(project, "description", "project")
        == python["registry_description"],
        "Python registry description disagrees with support matrix",
    )

    wasm = surfaces["wasm"]
    require(
        wasm["import_name"] == wasm["package"],
        "npm package and JavaScript import names disagree",
    )

    root_readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
    require(
        f"**{matrix['positioning'].replace('pdfplumber', '`pdfplumber`')}**"
        in root_readme,
        "README.md opening disagrees with canonical positioning",
    )
    root_phrases = (
        f"Release `{matrix['release_version']}`",
        f"Rust crate `{rust['package']}` (import `{rust['import_name']}`) is {rust['maturity']}",
        f"Python distribution `{python['package']}` (import `{python['import_name']}`) is {python['maturity']}",
        f"CLI crate `{cli['package']}` installs `{cli['executable']}` and is {cli['maturity']}",
        f"npm package `{wasm['package']}` is {wasm['maturity']}",
        f"`{matrix['license']}`",
        matrix["repository"],
        f"(docs/releases/v{matrix['release_version']}.md)",
    )
    for phrase in root_phrases:
        require(phrase in root_readme, f"README.md lacks {phrase!r}")

    readme_phrases = {
        "python": (
            f"Distribution `{python['package']}` installs import package `{python['import_name']}`",
            f"native module `{python['native_module']}`",
            f"Release `{matrix['release_version']}` is {python['maturity']}",
        ),
        "cli": (
            f"Crate `{cli['package']}` installs executable `{cli['executable']}`",
            f"Release `{matrix['release_version']}` is {cli['maturity']}",
        ),
        "wasm": (
            f"npm package and import name are `{wasm['package']}`",
            f"Release `{matrix['release_version']}` is {wasm['maturity']}",
        ),
    }
    for surface_id, phrases in readme_phrases.items():
        surface = surfaces[surface_id]
        readme = repository_path(surface["readme"], f"{surface_id} readme").read_text(
            encoding="utf-8"
        )
        for phrase in (*phrases, f"`{matrix['license']}`", matrix["repository"]):
            require(
                phrase in readme,
                f"{surface['readme']} lacks {phrase!r}",
            )
        require(
            surface["registry_description"] in readme.replace("`", ""),
            f"{surface['readme']} opening disagrees with registry description",
        )

    release_notes_path = repository_path(matrix["release_notes"], "release notes")
    require(release_notes_path.is_file(), "versioned release notes are missing")
    require(
        release_notes_path.read_text(encoding="utf-8") == render_release_notes(matrix),
        f"{matrix['release_notes']} is stale",
    )

    support_doc = (REPO_ROOT / "docs" / "support.md").read_text(encoding="utf-8")
    for value in (
        matrix["release_version"],
        matrix["license"],
        matrix["repository"],
        matrix["positioning"],
        *(surface["package"] for surface in matrix["surfaces"]),
        *(surface["registry_description"] for surface in matrix["surfaces"]),
    ):
        require(value in support_doc, f"docs/support.md lacks {value!r}")


def tar_member_bytes(archive: tarfile.TarFile, name: str) -> bytes:
    member = archive.getmember(name)
    extracted = archive.extractfile(member)
    require(extracted is not None, f"cannot read archive member {name}")
    return extracted.read()


def check_rust(matrix: dict[str, Any], paths: list[Path]) -> None:
    surfaces = surfaces_by_id(matrix)
    policy = load_toml(LICENSE_POLICY_PATH)
    expected = set(policy.get("rust_packages", []))
    require(bool(expected), "license policy Rust package list is missing")
    workspace = load_toml(REPO_ROOT / "Cargo.toml")
    source_descriptions = {}
    for member in workspace.get("workspace", {}).get("members", []):
        manifest = load_toml(REPO_ROOT / member / "Cargo.toml")
        package = manifest.get("package", {})
        if package.get("name") in expected:
            source_descriptions[package["name"]] = package.get("description")
    require(
        set(source_descriptions) == expected,
        "cannot resolve every publishable Rust package description",
    )
    found: set[str] = set()
    for path in paths:
        require(path.is_file(), f"Rust artifact does not exist: {path}")
        with tarfile.open(path, "r:gz") as archive:
            manifests = [
                name
                for name in archive.getnames()
                if name.count("/") == 1 and name.endswith("/Cargo.toml")
            ]
            require(len(manifests) == 1, f"{path} has ambiguous Cargo.toml")
            manifest = tomllib.loads(
                tar_member_bytes(archive, manifests[0]).decode("utf-8")
            )
            package = manifest.get("package", {})
            name = package.get("name")
            require(name in expected, f"{path} has unexpected package {name!r}")
            require(name not in found, f"duplicate Rust artifact for {name}")
            surface = (
                surfaces["cli"]
                if name == surfaces["cli"]["package"]
                else surfaces["rust"]
            )
            require(
                package.get("version") == matrix["release_version"],
                f"{name} version mismatch",
            )
            require(
                package.get("license") == matrix["license"], f"{name} license mismatch"
            )
            require(
                package.get("repository") == matrix["repository"],
                f"{name} repository mismatch",
            )
            require(
                package_maturity(package, str(path)) == surface["maturity"],
                f"{name} maturity mismatch",
            )
            description = require_string(package, "description", str(path))
            require(
                description == source_descriptions[name],
                f"{name} packaged description disagrees with source",
            )
            validate_public_description(
                description,
                surface["maturity"],
                f"{name} packaged description",
            )
            if name == surfaces["cli"]["package"]:
                require(
                    [target.get("name") for target in manifest.get("bin", [])]
                    == [surfaces["cli"]["executable"]],
                    "packaged CLI executable mismatch",
                )
            found.add(name)
    require(found == expected, f"Rust artifacts {sorted(found)} != {sorted(expected)}")


def parsed_metadata(content: bytes, context: str) -> Any:
    try:
        return Parser().parsestr(content.decode("utf-8"))
    except UnicodeDecodeError as error:
        raise MetadataError(f"{context} metadata is not UTF-8: {error}") from error


def check_python_metadata(matrix: dict[str, Any], metadata: Any, context: str) -> None:
    python = surfaces_by_id(matrix)["python"]
    require(metadata.get("Name") == python["package"], f"{context} name mismatch")
    require(
        metadata.get("Version") == matrix["release_version"],
        f"{context} version mismatch",
    )
    license_expression = metadata.get("License-Expression") or metadata.get("License")
    require(
        license_expression == matrix["license"],
        f"{context} license mismatch",
    )
    require(
        PYTHON_MATURITY_CLASSIFIERS[python["maturity"]]
        in (metadata.get_all("Classifier") or []),
        f"{context} maturity classifier mismatch",
    )
    project_urls = metadata.get_all("Project-URL") or []
    require(
        f"Repository, {matrix['repository']}" in project_urls,
        f"{context} repository URL mismatch",
    )
    require(
        metadata.get("Summary") == python["registry_description"],
        f"{context} summary disagrees with support matrix",
    )


def check_python(matrix: dict[str, Any], paths: list[Path]) -> None:
    python = surfaces_by_id(matrix)["python"]
    wheels = [path for path in paths if path.name.endswith(".whl")]
    sdists = [path for path in paths if path.name.endswith(".tar.gz")]
    require(bool(wheels), "no Python wheel supplied")
    require(bool(sdists), "no Python source distribution supplied")
    require(len(wheels) + len(sdists) == len(paths), "unknown Python artifact supplied")

    for path in wheels:
        require(path.is_file(), f"Python wheel does not exist: {path}")
        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            metadata_names = [
                name for name in names if name.endswith(".dist-info/METADATA")
            ]
            require(len(metadata_names) == 1, f"{path} has ambiguous METADATA")
            check_python_metadata(
                matrix,
                parsed_metadata(archive.read(metadata_names[0]), str(path)),
                str(path),
            )
            require(
                f"{python['import_name']}/__init__.py" in names,
                f"{path} lacks import package {python['import_name']}",
            )
            native_prefix = f"{python['native_module'].replace('.', '/')}"
            require(
                any(name.startswith(native_prefix) for name in names),
                f"{path} lacks native module {python['native_module']}",
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
                matrix,
                parsed_metadata(
                    tar_member_bytes(archive, f"{root}/PKG-INFO"), str(path)
                ),
                str(path),
            )


def normalized_repository(value: Any) -> str:
    if isinstance(value, dict):
        value = value.get("url")
    require(isinstance(value, str), "package repository must be a string or URL table")
    normalized = value.removeprefix("git+")
    normalized = normalized.removesuffix(".git")
    return normalized


def check_npm(matrix: dict[str, Any], directory: Path) -> None:
    require(directory.is_dir(), f"npm package directory does not exist: {directory}")
    try:
        package = json.loads((directory / "package.json").read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise MetadataError(f"cannot read npm package.json: {error}") from error
    wasm = surfaces_by_id(matrix)["wasm"]
    require(package.get("name") == wasm["package"], "npm package name mismatch")
    require(
        package.get("version") == matrix["release_version"],
        "npm package version mismatch",
    )
    require(package.get("license") == matrix["license"], "npm package license mismatch")
    require(
        normalized_repository(package.get("repository")) == matrix["repository"],
        "npm package repository mismatch",
    )
    require(
        package.get("description") == wasm["registry_description"],
        "npm package description disagrees with support matrix",
    )
    readme = (directory / "README.md").read_text(encoding="utf-8")
    for phrase in (
        f"npm package and import name are `{wasm['package']}`",
        f"Release `{matrix['release_version']}` is {wasm['maturity']}",
    ):
        require(phrase in readme, f"npm README lacks {phrase!r}")


def write_release_outputs(
    matrix: dict[str, Any],
    tag: str,
    github_output: Path | None,
) -> None:
    expected_tag = f"v{matrix['release_version']}"
    require(tag == expected_tag, f"release tag {tag} != source {expected_tag}")
    require(github_output is not None, "--github-output is required with --release-tag")
    release_notes = repository_path(matrix["release_notes"], "release notes")
    require(release_notes.is_file(), "release notes file is missing")
    prerelease = str(matrix["github_prerelease"]).lower()
    try:
        with github_output.open("a", encoding="utf-8") as output:
            output.write(f"release-notes={matrix['release_notes']}\n")
            output.write(f"prerelease={prerelease}\n")
    except OSError as error:
        raise MetadataError(
            f"cannot write GitHub output {github_output}: {error}"
        ) from error


def main() -> int:
    args = parse_args()
    try:
        matrix = load_matrix()
        if args.source:
            require(
                args.github_output is None,
                "--github-output is only valid with --release-tag",
            )
            check_source(matrix)
            family = "source"
        elif args.rust:
            require(
                args.github_output is None,
                "--github-output is only valid with --release-tag",
            )
            check_rust(matrix, args.rust)
            family = "Rust"
        elif args.python:
            require(
                args.github_output is None,
                "--github-output is only valid with --release-tag",
            )
            check_python(matrix, args.python)
            family = "Python"
        elif args.npm:
            require(
                args.github_output is None,
                "--github-output is only valid with --release-tag",
            )
            check_npm(matrix, args.npm)
            family = "npm"
        else:
            check_source(matrix)
            write_release_outputs(matrix, args.release_tag, args.github_output)
            family = "release"
    except (
        KeyError,
        MetadataError,
        OSError,
        tarfile.TarError,
        zipfile.BadZipFile,
    ) as error:
        print(f"package metadata check failed: {error}", file=sys.stderr)
        return 1
    print(f"{family} package metadata verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
