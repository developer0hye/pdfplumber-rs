"""Import and verify the exact pinned upstream PDF fixture corpus."""

from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REVISION_PATTERN = re.compile(r"^[0-9a-f]{40}$")


class CorpusMismatch(RuntimeError):
    """The source or committed fixture corpus differs from the pinned tree."""


@dataclass(frozen=True)
class Fingerprint:
    sha256: str
    file_count: int


@dataclass(frozen=True)
class CorpusConfig:
    project: str
    version: str
    tag: str
    commit: str
    repository: str
    source_root: Path
    destination_root: Path
    sha256: str
    file_count: int


def load_manifest(path: Path) -> CorpusConfig:
    """Load and validate the target-bound fixture import manifest."""

    try:
        with path.open("rb") as manifest_file:
            data = tomllib.load(manifest_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CorpusMismatch(f"cannot read fixture manifest: {path}") from error
    if data.get("schema_version") != 1:
        raise CorpusMismatch(f"unsupported fixture-manifest schema in {path}")
    source = _table(data, "source", path)
    corpus = _table(data, "corpus", path)
    config = CorpusConfig(
        project=_string(source, "project", path),
        version=_string(source, "version", path),
        tag=_string(source, "tag", path),
        commit=_string(source, "commit", path),
        repository=_string(source, "repository", path),
        source_root=Path(_string(corpus, "source_root", path)),
        destination_root=Path(_string(corpus, "destination_root", path)),
        sha256=_string(corpus, "sha256", path),
        file_count=_integer(corpus, "file_count", path),
    )
    if not REVISION_PATTERN.fullmatch(config.commit):
        raise CorpusMismatch(f"source.commit is not an immutable revision in {path}")
    if not SHA256_PATTERN.fullmatch(config.sha256):
        raise CorpusMismatch(f"corpus.sha256 is invalid in {path}")
    if config.file_count < 1:
        raise CorpusMismatch(f"corpus.file_count must be positive in {path}")
    _require_safe_relative_path(config.source_root, "corpus.source_root")
    _require_safe_relative_path(
        config.destination_root, "corpus.destination_root"
    )
    return config


def fingerprint_pdf_tree(base: Path, source_root: Path) -> Fingerprint:
    """Hash all PDF bytes and their source-relative directory names."""

    _require_safe_relative_path(source_root, "source root")
    tree_root = base / source_root
    if not tree_root.is_dir():
        raise CorpusMismatch(f"PDF source tree is missing: {tree_root}")
    files = sorted(
        path
        for path in tree_root.rglob("*")
        if path.is_file() and path.suffix == ".pdf"
    )
    digest = hashlib.sha256()
    for path in files:
        relative = path.relative_to(base).as_posix().encode("utf-8")
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return Fingerprint(sha256=digest.hexdigest(), file_count=len(files))


def verify_source_checkout(checkout: Path, config: CorpusConfig) -> Fingerprint:
    """Require the configured Git commit and its exact PDF path/byte tree."""

    commit = _git(checkout, "rev-parse", "HEAD")
    if commit != config.commit:
        raise CorpusMismatch(
            f"upstream checkout commit {commit} does not match {config.commit}"
        )
    fingerprint = fingerprint_pdf_tree(checkout, config.source_root)
    _verify_fingerprint(fingerprint, config, "upstream checkout")
    return fingerprint


def verify_import(repo_root: Path, config: CorpusConfig) -> Fingerprint:
    """Verify the committed import without network or an upstream checkout."""

    corpus_root = repo_root / config.destination_root
    unexpected = sorted(
        path.relative_to(corpus_root).as_posix()
        for path in corpus_root.rglob("*")
        if path.is_file() and path.suffix != ".pdf"
    ) if corpus_root.is_dir() else []
    if unexpected:
        raise CorpusMismatch(
            "committed corpus contains non-PDF files: " + ", ".join(unexpected)
        )
    fingerprint = fingerprint_pdf_tree(corpus_root, config.source_root)
    _verify_fingerprint(fingerprint, config, "committed corpus")
    return fingerprint


def materialize_corpus(
    checkout: Path,
    repo_root: Path,
    config: CorpusConfig,
) -> Fingerprint:
    """Copy a verified source tree while retaining every upstream path."""

    fingerprint = verify_source_checkout(checkout, config)
    destination = repo_root / config.destination_root
    if destination.exists():
        return verify_import(repo_root, config)
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(
            prefix=f".{destination.name}.",
            dir=destination.parent,
        )
    )
    try:
        source_tree = checkout / config.source_root
        for source in sorted(source_tree.rglob("*.pdf")):
            relative = source.relative_to(checkout)
            target = staging / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        staged = fingerprint_pdf_tree(staging, config.source_root)
        _verify_fingerprint(staged, config, "staged corpus")
        staging.rename(destination)
    except BaseException:
        if staging.exists():
            shutil.rmtree(staging)
        raise
    return fingerprint


def prepare_cache(cache: Path, config: CorpusConfig) -> None:
    """Create or refresh a clean checkout of the exact pinned source revision."""

    if cache.exists() and not (cache / ".git").is_dir():
        raise CorpusMismatch(
            f"managed cache exists but is not a Git checkout: {cache}"
        )
    if not cache.exists():
        cache.parent.mkdir(parents=True, exist_ok=True)
        _run(
            "git",
            "clone",
            "--filter=blob:none",
            "--depth",
            "1",
            "--branch",
            config.tag,
            config.repository,
            str(cache),
        )
    remote = _run("git", "-C", str(cache), "remote", "get-url", "origin").stdout
    if _normalized_repository(remote.strip()) != _normalized_repository(
        config.repository
    ):
        raise CorpusMismatch(
            f"managed cache origin {remote.strip()} does not match "
            f"{config.repository}"
        )
    status = _run("git", "-C", str(cache), "status", "--porcelain").stdout
    if status.strip():
        raise CorpusMismatch(f"managed cache has local changes: {cache}")
    _run("git", "-C", str(cache), "fetch", "--depth", "1", "origin", config.tag)
    _run("git", "-C", str(cache), "checkout", "--detach", config.commit)


def _verify_fingerprint(
    fingerprint: Fingerprint,
    config: CorpusConfig,
    label: str,
) -> None:
    if (
        fingerprint.sha256 != config.sha256
        or fingerprint.file_count != config.file_count
    ):
        raise CorpusMismatch(
            f"{label} fingerprint mismatch: expected {config.file_count} files "
            f"and {config.sha256}, got {fingerprint.file_count} files and "
            f"{fingerprint.sha256}"
        )


def _require_safe_relative_path(path: Path, label: str) -> None:
    pure_path = PurePosixPath(path.as_posix())
    if (
        path.is_absolute()
        or not pure_path.parts
        or ".." in pure_path.parts
        or pure_path == PurePosixPath(".")
    ):
        raise CorpusMismatch(f"{label} must be a safe relative path: {path}")


def _git(checkout: Path, *arguments: str) -> str:
    try:
        return _run("git", "-C", str(checkout), *arguments).stdout.strip()
    except CorpusMismatch as error:
        raise CorpusMismatch(
            f"cannot inspect upstream Git checkout at {checkout}"
        ) from error


def _run(*command: str) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise CorpusMismatch(
            "command failed while preparing upstream fixtures: "
            + " ".join(command)
        ) from error


def _normalized_repository(value: str) -> str:
    return value.removesuffix(".git").rstrip("/")


def _table(
    data: dict[str, object], key: str, path: Path
) -> dict[str, object]:
    value = data.get(key)
    if not isinstance(value, dict):
        raise CorpusMismatch(f"{key} must be a table in {path}")
    return value


def _string(data: dict[str, object], key: str, path: Path) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise CorpusMismatch(f"{key} must be a non-empty string in {path}")
    return value


def _integer(data: dict[str, object], key: str, path: Path) -> int:
    value = data.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        raise CorpusMismatch(f"{key} must be an integer in {path}")
    return value
