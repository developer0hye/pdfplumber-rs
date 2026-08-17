"""Pinned-source and result contracts for the upstream v0.11.10 test suite."""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path


SOURCE_METADATA_NAME: str = ".pdfplumber-upstream-source.json"
_IGNORED_PARTS: frozenset[str] = frozenset({".pytest_cache", "__pycache__"})
_PRD_TASK_PATTERN: re.Pattern[str] = re.compile(
    r"^- \[([ xX])\] \*\*([A-Z][A-Z0-9-]*-[0-9]+)\*\*"
)


class SuiteSourceMismatch(RuntimeError):
    """The materialized tests are not the pinned upstream source tree."""


class UnsupportedManifestError(RuntimeError):
    """The unsupported-test manifest is malformed or ambiguous."""


@dataclass(frozen=True)
class SourceConfig:
    project: str
    version: str
    tag: str
    commit: str
    repository: str
    tests_tree: str
    suite_paths: tuple[Path, ...]
    tests_sha256: str
    tests_file_count: int
    requirements_path: Path
    requirements_sha256: str
    external_commands: tuple[str, ...]


@dataclass(frozen=True)
class Fingerprint:
    sha256: str
    file_count: int


@dataclass(frozen=True)
class UnsupportedTest:
    nodeid: str
    task_id: str
    reason: str


@dataclass(frozen=True)
class UnsupportedManifest:
    version: str
    commit: str
    tests: tuple[UnsupportedTest, ...]


@dataclass(frozen=True)
class ResultClassification:
    known_unsupported: tuple[str, ...]
    unlisted_failures: tuple[str, ...]
    stale_unsupported: tuple[str, ...]
    uncollected_unsupported: tuple[str, ...]
    exit_code: int


def load_source_manifest(path: Path) -> SourceConfig:
    data = _load_toml(path)
    if data.get("schema_version") != 1:
        raise SuiteSourceMismatch(f"unsupported source-manifest schema in {path}")
    source = _table(data, "source", path)
    tests = _table(data, "tests", path)
    environment = _table(data, "test_environment", path)
    return SourceConfig(
        project=_string(source, "project", path),
        version=_string(source, "version", path),
        tag=_string(source, "tag", path),
        commit=_string(source, "commit", path),
        repository=_string(source, "repository", path),
        tests_tree=_string(tests, "git_tree", path),
        suite_paths=_paths(tests, "paths", path),
        tests_sha256=_string(tests, "sha256", path),
        tests_file_count=_integer(tests, "file_count", path),
        requirements_path=Path(_string(environment, "requirements", path)),
        requirements_sha256=_string(environment, "requirements_sha256", path),
        external_commands=_strings(environment, "external_commands", path),
    )


def load_unsupported_manifest(path: Path) -> UnsupportedManifest:
    data = _load_toml(path)
    if data.get("schema_version") != 1:
        raise UnsupportedManifestError(
            f"unsupported unsupported-test manifest schema in {path}"
        )
    target = _table(data, "target", path)
    raw_tests = data.get("test", [])
    if not isinstance(raw_tests, list):
        raise UnsupportedManifestError(f"test must be an array of tables in {path}")
    tests: list[UnsupportedTest] = []
    seen: set[str] = set()
    for index, raw in enumerate(raw_tests):
        if not isinstance(raw, dict):
            raise UnsupportedManifestError(f"test[{index}] must be a table in {path}")
        nodeid = _string(raw, "nodeid", path)
        if nodeid in seen:
            raise UnsupportedManifestError(f"duplicate unsupported nodeid: {nodeid}")
        seen.add(nodeid)
        tests.append(
            UnsupportedTest(
                nodeid=nodeid,
                task_id=_string(raw, "task_id", path),
                reason=_string(raw, "reason", path),
            )
        )
    return UnsupportedManifest(
        version=_string(target, "version", path),
        commit=_string(target, "commit", path),
        tests=tuple(sorted(tests, key=lambda test: test.nodeid)),
    )


def tree_fingerprint(root: Path) -> Fingerprint:
    return _fingerprint_files(
        root,
        [path for path in root.rglob("*") if path.is_file()],
    )


def verify_git_checkout(checkout: Path, config: SourceConfig) -> None:
    commit = _git(checkout, "rev-parse", "HEAD")
    if commit != config.commit:
        raise SuiteSourceMismatch(
            f"upstream checkout commit {commit} does not match {config.commit}"
        )
    tests_tree = _git(checkout, "rev-parse", "HEAD:tests")
    if tests_tree != config.tests_tree:
        raise SuiteSourceMismatch(
            f"upstream tests tree {tests_tree} does not match {config.tests_tree}"
        )
    fingerprint = tree_fingerprint_paths(checkout, config.suite_paths)
    _verify_fingerprint(fingerprint, config, "upstream checkout")


def materialize_suite(
    checkout: Path,
    destination: Path,
    config: SourceConfig,
) -> None:
    verify_git_checkout(checkout, config)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        metadata = destination / SOURCE_METADATA_NAME
        if not metadata.is_file():
            raise SuiteSourceMismatch(
                f"refusing to replace {destination}: source metadata is missing"
            )
        shutil.rmtree(destination)
    destination.mkdir()
    for relative in config.suite_paths:
        source = checkout / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(source, target)
        elif source.is_file():
            shutil.copy2(source, target)
        else:
            raise SuiteSourceMismatch(f"configured suite path is missing: {relative}")
    fingerprint = tree_fingerprint(destination)
    _verify_fingerprint(fingerprint, config, "materialized suite")
    metadata = {
        "commit": config.commit,
        "tests_tree": config.tests_tree,
        "tests_sha256": fingerprint.sha256,
        "tests_file_count": fingerprint.file_count,
    }
    (destination / SOURCE_METADATA_NAME).write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify_materialized_suite(root: Path, config: SourceConfig) -> None:
    metadata_path = root / SOURCE_METADATA_NAME
    if not metadata_path.is_file():
        raise SuiteSourceMismatch(f"source metadata is missing: {metadata_path}")
    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as error:
        raise SuiteSourceMismatch(f"invalid source metadata: {metadata_path}") from error
    expected = {
        "commit": config.commit,
        "tests_tree": config.tests_tree,
        "tests_sha256": config.tests_sha256,
        "tests_file_count": config.tests_file_count,
    }
    if metadata != expected:
        raise SuiteSourceMismatch(
            f"source metadata does not match the pinned manifest: {metadata_path}"
        )
    fingerprint = tree_fingerprint(root)
    _verify_fingerprint(fingerprint, config, "materialized suite")


def tree_fingerprint_paths(root: Path, paths: tuple[Path, ...]) -> Fingerprint:
    """Fingerprint configured files while retaining paths relative to root."""
    files: list[Path] = []
    for relative in paths:
        source = root / relative
        if source.is_dir():
            files.extend(path for path in source.rglob("*") if path.is_file())
        elif source.is_file():
            files.append(source)
        else:
            raise SuiteSourceMismatch(f"configured suite path is missing: {relative}")
    return _fingerprint_files(root, files)


def classify_results(
    *,
    collected: tuple[str, ...],
    failed: tuple[str, ...],
    manifest: UnsupportedManifest,
    pytest_exit_code: int,
) -> ResultClassification:
    collected_set = set(collected)
    failed_set = set(failed)
    listed_set = {test.nodeid for test in manifest.tests}
    known = tuple(sorted(failed_set & listed_set))
    unlisted = tuple(sorted(failed_set - listed_set))
    stale = tuple(sorted((listed_set & collected_set) - failed_set))
    uncollected = tuple(sorted(listed_set - collected_set))
    manifest_problem = bool(stale or uncollected)
    exit_code = pytest_exit_code if pytest_exit_code != 0 else int(manifest_problem)
    return ResultClassification(
        known_unsupported=known,
        unlisted_failures=unlisted,
        stale_unsupported=stale,
        uncollected_unsupported=uncollected,
        exit_code=exit_code,
    )


def validate_unsupported_task_links(
    manifest: UnsupportedManifest,
    prd_path: Path,
) -> None:
    """Require every unsupported node to reference an open section 8 task."""
    try:
        lines = prd_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise UnsupportedManifestError(f"cannot read PRD: {prd_path}") from error
    section = _section_eight(lines, prd_path)
    task_states: dict[str, bool] = {}
    for line in section:
        match = _PRD_TASK_PATTERN.match(line)
        if match is None:
            continue
        checked, task_id = match.groups()
        task_states[task_id] = checked.lower() == "x"

    for test in manifest.tests:
        checked = task_states.get(test.task_id)
        if checked is None:
            raise UnsupportedManifestError(
                f"unsupported test {test.nodeid} references unknown task "
                f"{test.task_id}"
            )
        if checked:
            raise UnsupportedManifestError(
                f"unsupported test {test.nodeid} references checked task "
                f"{test.task_id}"
            )


def _section_eight(lines: list[str], prd_path: Path) -> list[str]:
    try:
        start = next(
            index
            for index, line in enumerate(lines)
            if line.startswith("## 8.")
        )
        end = next(
            index
            for index, line in enumerate(lines[start + 1 :], start + 1)
            if line.startswith("## 9.")
        )
    except StopIteration as error:
        raise UnsupportedManifestError(
            f"cannot locate PRD sections 8 and 9 in {prd_path}"
        ) from error
    return lines[start + 1 : end]


def _verify_fingerprint(
    fingerprint: Fingerprint,
    config: SourceConfig,
    label: str,
) -> None:
    if (
        fingerprint.sha256 != config.tests_sha256
        or fingerprint.file_count != config.tests_file_count
    ):
        raise SuiteSourceMismatch(
            f"{label} content fingerprint {fingerprint.sha256}/"
            f"{fingerprint.file_count} does not match "
            f"{config.tests_sha256}/{config.tests_file_count}"
        )


def _is_source_file(relative: Path) -> bool:
    if relative.name == SOURCE_METADATA_NAME or relative.suffix == ".pyc":
        return False
    return not any(part in _IGNORED_PARTS for part in relative.parts)


def _fingerprint_files(root: Path, files: list[Path]) -> Fingerprint:
    digest = hashlib.sha256()
    included = sorted(
        path for path in files if _is_source_file(path.relative_to(root))
    )
    for path in included:
        relative = path.relative_to(root).as_posix().encode("utf-8")
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return Fingerprint(sha256=digest.hexdigest(), file_count=len(included))


def _git(checkout: Path, *arguments: str) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(checkout), *arguments],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise SuiteSourceMismatch(
            f"cannot inspect upstream Git checkout at {checkout}"
        ) from error
    return result.stdout.strip()


def _load_toml(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as handle:
            return tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SuiteSourceMismatch(f"cannot read manifest: {path}") from error


def _table(data: dict[str, object], key: str, path: Path) -> dict[str, object]:
    value = data.get(key)
    if not isinstance(value, dict):
        raise SuiteSourceMismatch(f"{key} must be a table in {path}")
    return value


def _string(data: dict[str, object], key: str, path: Path) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise SuiteSourceMismatch(f"{key} must be a non-empty string in {path}")
    return value


def _integer(data: dict[str, object], key: str, path: Path) -> int:
    value = data.get(key)
    if not isinstance(value, int) or value < 1:
        raise SuiteSourceMismatch(f"{key} must be a positive integer in {path}")
    return value


def _paths(data: dict[str, object], key: str, path: Path) -> tuple[Path, ...]:
    value = data.get(key)
    if not isinstance(value, list) or not value:
        raise SuiteSourceMismatch(f"{key} must be a non-empty array in {path}")
    parsed: list[Path] = []
    for entry in value:
        if not isinstance(entry, str) or not entry:
            raise SuiteSourceMismatch(f"{key} entries must be strings in {path}")
        relative = Path(entry)
        if relative.is_absolute() or ".." in relative.parts:
            raise SuiteSourceMismatch(f"unsafe suite path {entry!r} in {path}")
        if relative in parsed:
            raise SuiteSourceMismatch(f"duplicate suite path {entry!r} in {path}")
        parsed.append(relative)
    if Path("tests") not in parsed:
        raise SuiteSourceMismatch(f"{key} must include the upstream tests directory")
    return tuple(parsed)


def _strings(data: dict[str, object], key: str, path: Path) -> tuple[str, ...]:
    value = data.get(key)
    if not isinstance(value, list):
        raise SuiteSourceMismatch(f"{key} must be an array in {path}")
    if any(not isinstance(entry, str) or not entry for entry in value):
        raise SuiteSourceMismatch(f"{key} entries must be strings in {path}")
    strings = tuple(value)
    if len(strings) != len(set(strings)):
        raise SuiteSourceMismatch(f"{key} contains duplicate entries in {path}")
    return strings
