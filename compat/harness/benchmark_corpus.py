"""Validate and render the versioned redistributable benchmark corpus."""

from __future__ import annotations

import re
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import corpus_index

CORPUS_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
FIXTURE_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SEMVER_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
REQUIRED_SEMANTIC_CLASSES = frozenset(
    {
        "cjk",
        "encrypted",
        "graphics-heavy",
        "image-heavy",
        "malformed",
        "right-to-left",
        "table-heavy",
        "text-only",
        "word-geometry",
    }
)
SIZE_CLASSES = frozenset({"small", "medium", "large"})


class BenchmarkCorpusError(ValueError):
    """The benchmark selection is incomplete, stale, or ambiguous."""


@dataclass(frozen=True)
class BenchmarkFixture:
    id: str
    path: str
    sha256: str
    source: str
    semantic_classes: tuple[str, ...]
    size_class: str
    page_count: int
    byte_size: int
    password: str | None
    description: str


@dataclass(frozen=True)
class BenchmarkCorpus:
    id: str
    release: str
    fixture_index: str
    reference: str
    small_max_bytes: int
    large_min_bytes: int
    fixtures: tuple[BenchmarkFixture, ...]

    def semantic_classes(self) -> frozenset[str]:
        """Return every semantic workload class in the selection."""

        return frozenset(
            semantic_class
            for fixture in self.fixtures
            for semantic_class in fixture.semantic_classes
        )

    def size_classes(self) -> frozenset[str]:
        """Return every measured size class in the selection."""

        return frozenset(fixture.size_class for fixture in self.fixtures)


def load_manifest(path: Path) -> dict[str, object]:
    """Load one benchmark-corpus TOML manifest."""

    try:
        with path.open("rb") as manifest_file:
            return tomllib.load(manifest_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkCorpusError(
            f"cannot read benchmark corpus manifest: {path}"
        ) from error


def audit_repository(
    repo_root: Path,
    manifest_path: Path,
    registry_path: Path,
) -> BenchmarkCorpus:
    """Validate the selection against the licensed repository corpus."""

    index = corpus_index.audit_repository(repo_root, registry_path)
    corpus = validate_manifest(load_manifest(manifest_path), index, repo_root)
    try:
        expected_registry = registry_path.relative_to(repo_root).as_posix()
    except ValueError as error:
        raise BenchmarkCorpusError(
            "fixture registry must be inside the repository"
        ) from error
    if corpus.fixture_index != expected_registry:
        raise BenchmarkCorpusError(
            "manifest fixture_index does not name the audited corpus index"
        )
    return corpus


def validate_manifest(
    manifest: Mapping[str, object],
    index: corpus_index.CorpusIndex,
    repo_root: Path,
) -> BenchmarkCorpus:
    """Validate one benchmark manifest against a trusted corpus index."""

    schema = manifest.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkCorpusError("schema.version must be 1")

    raw_corpus = manifest.get("corpus")
    if not isinstance(raw_corpus, dict):
        raise BenchmarkCorpusError("corpus must be one table")
    corpus_id = _required_string(raw_corpus, "id", "corpus")
    if not CORPUS_ID_PATTERN.fullmatch(corpus_id):
        raise BenchmarkCorpusError(f"invalid corpus id: {corpus_id}")
    release = _required_string(raw_corpus, "release", "corpus")
    if not SEMVER_PATTERN.fullmatch(release):
        raise BenchmarkCorpusError(f"invalid corpus release: {release}")
    fixture_index = _required_repository_path(
        raw_corpus,
        "fixture_index",
        "corpus",
        repo_root,
    )
    reference = _required_repository_path(
        raw_corpus,
        "reference",
        "corpus",
        repo_root,
    )
    small_max_bytes = _required_positive_integer(
        raw_corpus,
        "small_max_bytes",
        "corpus",
    )
    large_min_bytes = _required_positive_integer(
        raw_corpus,
        "large_min_bytes",
        "corpus",
    )
    if small_max_bytes >= large_min_bytes:
        raise BenchmarkCorpusError("small_max_bytes must be less than large_min_bytes")

    raw_fixtures = manifest.get("fixtures")
    if not isinstance(raw_fixtures, list) or not raw_fixtures:
        raise BenchmarkCorpusError("fixtures must be a non-empty array")

    fixtures: list[BenchmarkFixture] = []
    fixture_ids: set[str] = set()
    fixture_paths: set[str] = set()
    for position, raw_fixture in enumerate(raw_fixtures, start=1):
        if not isinstance(raw_fixture, dict):
            raise BenchmarkCorpusError(f"fixture {position} must be a table")
        fixture = _validate_fixture(
            raw_fixture,
            index,
            repo_root,
            small_max_bytes,
            large_min_bytes,
        )
        if fixture.id in fixture_ids:
            raise BenchmarkCorpusError(f"duplicate fixture id: {fixture.id}")
        if fixture.path in fixture_paths:
            raise BenchmarkCorpusError(f"duplicate fixture path: {fixture.path}")
        fixture_ids.add(fixture.id)
        fixture_paths.add(fixture.path)
        fixtures.append(fixture)

    covered_classes = frozenset(
        semantic_class
        for fixture in fixtures
        for semantic_class in fixture.semantic_classes
    )
    missing_classes = REQUIRED_SEMANTIC_CLASSES - covered_classes
    if missing_classes:
        raise BenchmarkCorpusError(
            "missing semantic classes: " + ", ".join(sorted(missing_classes))
        )
    covered_sizes = frozenset(fixture.size_class for fixture in fixtures)
    missing_sizes = {"small", "large"} - covered_sizes
    if missing_sizes:
        raise BenchmarkCorpusError(
            "missing size classes: " + ", ".join(sorted(missing_sizes))
        )

    return BenchmarkCorpus(
        id=corpus_id,
        release=release,
        fixture_index=fixture_index,
        reference=reference,
        small_max_bytes=small_max_bytes,
        large_min_bytes=large_min_bytes,
        fixtures=tuple(sorted(fixtures, key=lambda fixture: fixture.id)),
    )


def _validate_fixture(
    raw_fixture: Mapping[str, object],
    index: corpus_index.CorpusIndex,
    repo_root: Path,
    small_max_bytes: int,
    large_min_bytes: int,
) -> BenchmarkFixture:
    fixture_id = _required_string(raw_fixture, "id", "fixture")
    if not FIXTURE_ID_PATTERN.fullmatch(fixture_id):
        raise BenchmarkCorpusError(f"invalid fixture id: {fixture_id}")
    path = _required_string(raw_fixture, "path", f"fixture {fixture_id}")
    try:
        indexed = index.fixture(path)
    except corpus_index.CorpusIndexError as error:
        raise BenchmarkCorpusError(str(error)) from error
    digest = _required_string(raw_fixture, "sha256", f"fixture {fixture_id}")
    if digest != indexed.sha256:
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} digest disagrees with corpus index"
        )

    raw_classes = raw_fixture.get("semantic_classes")
    if not isinstance(raw_classes, list) or not raw_classes:
        raise BenchmarkCorpusError(f"fixture {fixture_id} needs semantic_classes")
    if not all(isinstance(item, str) for item in raw_classes):
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} has a non-string semantic class"
        )
    semantic_classes = tuple(raw_classes)
    if len(set(semantic_classes)) != len(semantic_classes):
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} has duplicate semantic classes"
        )
    if semantic_classes != tuple(sorted(semantic_classes)):
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} semantic_classes must be sorted"
        )
    for semantic_class in semantic_classes:
        if semantic_class not in REQUIRED_SEMANTIC_CLASSES:
            raise BenchmarkCorpusError(f"unknown semantic class: {semantic_class}")

    size_class = _required_string(
        raw_fixture,
        "size_class",
        f"fixture {fixture_id}",
    )
    if size_class not in SIZE_CLASSES:
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} has invalid size_class: {size_class}"
        )
    fixture_path = repo_root / path
    try:
        byte_size = fixture_path.stat().st_size
    except OSError as error:
        raise BenchmarkCorpusError(f"cannot stat fixture: {path}") from error
    measured_size = _size_class(
        byte_size,
        small_max_bytes,
        large_min_bytes,
    )
    if size_class != measured_size:
        if size_class == "large":
            reason = "does not meet large threshold"
        elif size_class == "small":
            reason = "exceeds small threshold"
        else:
            reason = f"measures as {measured_size}"
        raise BenchmarkCorpusError(f"fixture {fixture_id} {reason}")

    page_count = _required_positive_integer(
        raw_fixture,
        "page_count",
        f"fixture {fixture_id}; expected a positive page_count",
    )
    description = _required_string(
        raw_fixture,
        "description",
        f"fixture {fixture_id}",
    )
    raw_password = raw_fixture.get("password")
    password = raw_password if isinstance(raw_password, str) else None
    if "encrypted" in semantic_classes:
        if not password:
            raise BenchmarkCorpusError(
                f"encrypted fixture {fixture_id} needs a non-empty password"
            )
    elif raw_password is not None:
        raise BenchmarkCorpusError(
            f"fixture {fixture_id} has a password without encrypted class"
        )

    return BenchmarkFixture(
        id=fixture_id,
        path=path,
        sha256=digest,
        source=indexed.source,
        semantic_classes=semantic_classes,
        size_class=size_class,
        page_count=page_count,
        byte_size=byte_size,
        password=password,
        description=description,
    )


def _required_string(
    table: Mapping[str, object],
    key: str,
    context: str,
) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value:
        raise BenchmarkCorpusError(f"{context} needs a non-empty {key}")
    return value


def _required_repository_path(
    table: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    value = _required_string(table, key, context)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not (repo_root / path).is_file():
        raise BenchmarkCorpusError(f"{context} {key} is not a repository file: {value}")
    return path.as_posix()


def _required_positive_integer(
    table: Mapping[str, object],
    key: str,
    context: str,
) -> int:
    value = table.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise BenchmarkCorpusError(f"{context} needs a positive {key}")
    return value


def _size_class(
    byte_size: int,
    small_max_bytes: int,
    large_min_bytes: int,
) -> str:
    if byte_size <= small_max_bytes:
        return "small"
    if byte_size >= large_min_bytes:
        return "large"
    return "medium"


def render_markdown(corpus: BenchmarkCorpus) -> str:
    """Render the public, versioned selection without benchmark results."""

    rows = []
    for fixture in corpus.fixtures:
        access = f"password `{fixture.password}`" if fixture.password else "none"
        rows.append(
            "| "
            + " | ".join(
                (
                    f"`{fixture.id}`",
                    f"[`{fixture.path}`](../../{fixture.path})",
                    ", ".join(
                        f"`{semantic_class}`"
                        for semantic_class in fixture.semantic_classes
                    ),
                    f"`{fixture.size_class}`",
                    f"{fixture.byte_size:,}",
                    str(fixture.page_count),
                    access,
                    f"`{fixture.source}`",
                )
            )
            + " |"
        )

    required_classes = ", ".join(
        f"`{semantic_class}`" for semantic_class in sorted(REQUIRED_SEMANTIC_CLASSES)
    )
    total_bytes = sum(fixture.byte_size for fixture in corpus.fixtures)
    return "\n".join(
        (
            f"# Benchmark corpus {corpus.release}",
            "",
            (
                "This versioned selection defines inputs; it does not publish a "
                "timing or performance result. Every selected PDF is already "
                "covered by the "
                f"[licensed corpus index](../../{corpus.fixture_index}), and the "
                "manifest binds the "
                "selection to exact SHA-256 digests. "
                f"`{corpus.id}` contains {len(corpus.fixtures)} unique PDFs "
                f"totaling {total_bytes:,} bytes."
            ),
            "",
            (
                "Before any comparison is timed, `SCORE-002` must prove materially "
                "equivalent requested outputs and semantics. A failed or unsupported "
                "case is reported separately and cannot become a performance win."
            ),
            "",
            "## Coverage contract",
            "",
            f"Required semantic classes: {required_classes}.",
            "",
            (
                f"`small` means at most {corpus.small_max_bytes:,} bytes; `large` "
                f"means at least {corpus.large_min_bytes:,} bytes; values between "
                "those bounds are `medium`. The recorded page count is part of the "
                "digest-bound fixture description; byte classifications are checked "
                "directly from the committed files."
            ),
            "",
            "| ID | Fixture | Semantic classes | Size | Bytes | Pages | Access | Source |",
            "|---|---|---|---:|---:|---:|---|---|",
            *rows,
            "",
            "## Selection notes",
            "",
            *(
                f"- `{fixture.id}`: {fixture.description}"
                for fixture in corpus.fixtures
            ),
            "",
            (
                "The workload breadth follows the source-pinned corpus observation "
                f"in [`{corpus.reference}`](../../{corpus.reference}); no external "
                "project's self-published result is copied into this corpus "
                "definition."
            ),
            "",
            "## Verify",
            "",
            "```bash",
            "python3 scripts/check_fixture_licenses.py",
            "python3 scripts/check_corpus_index.py",
            "python3 scripts/generate_benchmark_corpus.py --check",
            "```",
            "",
        )
    )
