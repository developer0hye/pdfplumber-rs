#!/usr/bin/env python3
"""Generate or validate the versioned Python pdfplumber release matrix."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import compatibility_scorecard, corpus_index
from scripts import release_version

DEFAULT_SOURCE = REPO_ROOT / "compat" / "python-release-matrix-v0.3.0.toml"
SERIES_PATTERN = re.compile(r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")
SHA1_PATTERN = re.compile(r"^[0-9a-f]{40}$")
STATUS_ORDER = compatibility_scorecard.STATUSES


class PythonReleaseMatrixError(ValueError):
    """Release-specific evidence cannot be rendered without inference."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SOURCE,
        help="matrix source TOML (defaults to the current release source)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate that the committed matrix equals the rendered source",
    )
    return parser.parse_args()


def load_toml(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as source_file:
            value = tomllib.load(source_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise PythonReleaseMatrixError(
            f"cannot read matrix source {path}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise PythonReleaseMatrixError(f"matrix source is not a table: {path}")
    return value


def load_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PythonReleaseMatrixError(
            f"cannot read scorecard {path}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise PythonReleaseMatrixError(f"scorecard is not an object: {path}")
    return value


def required_string(data: Mapping[str, object], key: str, context: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise PythonReleaseMatrixError(f"{context} needs a non-empty {key}")
    return value


def repository_path(value: object, context: str) -> Path:
    if not isinstance(value, str) or not value:
        raise PythonReleaseMatrixError(f"{context} must be a repository-relative path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise PythonReleaseMatrixError(f"unsafe {context}: {value}")
    return REPO_ROOT / relative


def release_entries(source: Mapping[str, object]) -> tuple[Mapping[str, object], ...]:
    raw_releases = source.get("releases")
    if not isinstance(raw_releases, list) or not raw_releases:
        raise PythonReleaseMatrixError("matrix source needs a non-empty releases array")
    releases: list[Mapping[str, object]] = []
    for position, release in enumerate(raw_releases):
        if not isinstance(release, dict):
            raise PythonReleaseMatrixError(f"release entry {position} is not a table")
        releases.append(release)
    return tuple(releases)


def load_validated_scorecard(
    path: Path,
    *,
    release_version_value: str,
    release_tag: str,
    subject_version: str,
    upstream_repository: str,
) -> dict[str, object]:
    scorecard = load_json(path)
    corpus = _required_mapping(scorecard, "corpus", "scorecard")
    corpus_path = repository_path(corpus.get("index"), "scorecard corpus index")
    corpus_index_value = corpus_index.load_index(corpus_path)
    corpus_sha256 = hashlib.sha256(corpus_path.read_bytes()).hexdigest()
    compatibility_scorecard.validate(
        scorecard,
        corpus=corpus_index_value,
        corpus_sha256=corpus_sha256,
    )

    target = _required_mapping(scorecard, "target", "scorecard")
    target_version = required_string(target, "version", "scorecard target")
    if target_version != release_version_value:
        raise PythonReleaseMatrixError(
            f"observed release {release_version_value} differs from scorecard target "
            f"{target_version}"
        )
    if required_string(target, "tag", "scorecard target") != release_tag:
        raise PythonReleaseMatrixError(
            f"observed release {release_version_value} tag differs from scorecard target"
        )
    if required_string(target, "project", "scorecard target") != "pdfplumber":
        raise PythonReleaseMatrixError("scorecard target project must be pdfplumber")
    if required_string(target, "repository", "scorecard target") != upstream_repository:
        raise PythonReleaseMatrixError(
            "scorecard target repository differs from matrix source"
        )
    if (
        SHA1_PATTERN.fullmatch(required_string(target, "commit", "scorecard target"))
        is None
    ):
        raise PythonReleaseMatrixError("scorecard target commit is invalid")

    subject = _required_mapping(scorecard, "subject", "scorecard")
    if required_string(subject, "project", "scorecard subject") != "pdfplumber-rs":
        raise PythonReleaseMatrixError(
            "scorecard subject project must be pdfplumber-rs"
        )
    if required_string(subject, "version", "scorecard subject") != subject_version:
        raise PythonReleaseMatrixError(
            f"scorecard subject version differs from matrix subject {subject_version}"
        )
    if (
        SHA1_PATTERN.fullmatch(
            required_string(subject, "revision", "scorecard subject")
        )
        is None
    ):
        raise PythonReleaseMatrixError("scorecard subject revision is invalid")

    summary = _required_mapping(scorecard, "summary", "scorecard")
    counts = _required_mapping(summary, "status_counts", "scorecard summary")
    if set(counts) != set(STATUS_ORDER):
        raise PythonReleaseMatrixError(
            "scorecard status counts differ from the vocabulary"
        )
    for status in STATUS_ORDER:
        count = counts.get(status)
        if not isinstance(count, int) or isinstance(count, bool) or count < 0:
            raise PythonReleaseMatrixError(f"scorecard count for {status} is invalid")
    return scorecard


def build(source: Mapping[str, object]) -> dict[str, object]:
    if source.get("schema_version") != 1:
        raise PythonReleaseMatrixError("matrix source schema_version must be 1")
    subject_version = required_string(source, "subject_version", "matrix source")

    release_series = required_string(source, "release_series", "matrix source")
    if SERIES_PATTERN.fullmatch(release_series) is None:
        raise PythonReleaseMatrixError("release_series must use X.Y")
    upstream_repository = required_string(
        source,
        "upstream_repository",
        "matrix source",
    ).removesuffix("/")
    if upstream_repository != "https://github.com/jsvine/pdfplumber":
        raise PythonReleaseMatrixError(
            "upstream repository must be canonical pdfplumber"
        )
    release_index = required_string(source, "release_index", "matrix source")
    if release_index != f"{upstream_repository}/tags":
        raise PythonReleaseMatrixError(
            "release index must be the canonical upstream tag list"
        )

    rows: list[dict[str, object]] = []
    versions: list[str] = []
    for position, release in enumerate(release_entries(source)):
        context = f"release entry {position}"
        version = required_string(release, "version", context)
        tag = required_string(release, "tag", context)
        status = required_string(release, "status", context)
        if release_version.VERSION_PATTERN.fullmatch(version) is None:
            raise PythonReleaseMatrixError(f"{context} version must use X.Y.Z")
        if not version.startswith(f"{release_series}."):
            raise PythonReleaseMatrixError(
                f"release {version} is outside configured series {release_series}"
            )
        if tag != f"v{version}":
            raise PythonReleaseMatrixError(f"release {version} tag must be v{version}")

        row: dict[str, object] = {
            "version": version,
            "tag": tag,
            "status": status,
            "tag_url": f"{upstream_repository}/tree/{tag}",
        }
        if status == "observed":
            if set(release) != {"version", "tag", "status", "scorecard"}:
                raise PythonReleaseMatrixError(
                    f"observed release {version} has missing or unknown fields"
                )
            scorecard_path = repository_path(
                release.get("scorecard"),
                f"release {version} scorecard",
            )
            scorecard = load_validated_scorecard(
                scorecard_path,
                release_version_value=version,
                release_tag=tag,
                subject_version=subject_version,
                upstream_repository=upstream_repository,
            )
            row.update(
                {
                    "scorecard_path": scorecard_path.relative_to(REPO_ROOT).as_posix(),
                    "scorecard_sha256": hashlib.sha256(
                        scorecard_path.read_bytes()
                    ).hexdigest(),
                    "target_commit": _required_mapping(
                        scorecard,
                        "target",
                        "scorecard",
                    )["commit"],
                    "subject_revision": _required_mapping(
                        scorecard,
                        "subject",
                        "scorecard",
                    )["revision"],
                    "status_counts": dict(
                        _required_mapping(
                            _required_mapping(scorecard, "summary", "scorecard"),
                            "status_counts",
                            "scorecard summary",
                        )
                    ),
                }
            )
        elif status == "not_tested":
            if set(release) != {"version", "tag", "status", "reason"}:
                raise PythonReleaseMatrixError(
                    f"not-tested release {version} has missing or unknown fields"
                )
            row["reason"] = required_string(release, "reason", context)
        else:
            raise PythonReleaseMatrixError(
                f"release {version} status must be observed or not_tested"
            )
        versions.append(version)
        rows.append(row)

    if len(versions) != len(set(versions)):
        raise PythonReleaseMatrixError("matrix release versions must be unique")
    semantic_versions = [
        tuple(int(part) for part in version.split(".")) for version in versions
    ]
    if semantic_versions != sorted(semantic_versions, reverse=True):
        raise PythonReleaseMatrixError("matrix releases must be newest first")
    if not any(row["status"] == "observed" for row in rows):
        raise PythonReleaseMatrixError("matrix needs at least one observed release")

    return {
        "schema_version": 1,
        "subject_version": subject_version,
        "release_series": release_series,
        "upstream_repository": upstream_repository,
        "release_index": release_index,
        "releases": rows,
    }


def render(matrix: Mapping[str, object]) -> str:
    subject_version = required_string(matrix, "subject_version", "matrix")
    release_series = required_string(matrix, "release_series", "matrix")
    release_index = required_string(matrix, "release_index", "matrix")
    releases = _required_mapping_sequence(matrix, "releases", "matrix")

    lines = [
        f"# Python pdfplumber release matrix for pdfplumber-rs v{subject_version}",
        "",
        (
            "This versioned matrix keeps evidence separate for each exact Python "
            "`pdfplumber` release. It does not make a blanket compatibility claim. "
            "A result for one row never transfers to another release."
        ),
        "",
        (
            "The [compatibility terminology](terms.md) defines the required claim "
            "scope and outcome vocabulary. Unlisted releases are also not tested; "
            "absence from this table is not compatibility evidence."
        ),
        "",
        "## Scope",
        "",
        f"- Candidate release: `pdfplumber-rs` `{subject_version}`.",
        f"- Enumerated upstream series: Python `pdfplumber` `{release_series}.x`.",
        f"- Authoritative upstream inventory: [release tags]({release_index}).",
        "- Matrix source: `compat/python-release-matrix-v0.3.0.toml`.",
        "",
        "## Release matrix",
        "",
        "| Python pdfplumber release | Coverage | Evidence | Boundary |",
        "| --- | --- | --- | --- |",
    ]
    for release in releases:
        version = required_string(release, "version", "matrix release")
        tag_url = required_string(release, "tag_url", f"release {version}")
        if release.get("status") == "observed":
            scorecard_path = required_string(
                release,
                "scorecard_path",
                f"release {version}",
            )
            scorecard_link = scorecard_path.rsplit("/", maxsplit=1)[-1]
            counts = _required_mapping(release, "status_counts", f"release {version}")
            count_text = "; ".join(
                f"{status}={counts[status]}" for status in STATUS_ORDER
            )
            evidence = (
                f"[machine-readable scorecard]({scorecard_link}); `{count_text}`; "
                f"SHA-256 `{required_string(release, 'scorecard_sha256', f'release {version}')}`"
            )
            boundary = (
                "Release-specific observations include unsupported, failure, and "
                "untested outcomes; inspect the scorecard before any scoped claim."
            )
            coverage = "Observed"
        else:
            coverage = "Not tested — no release-specific scorecard"
            evidence = required_string(release, "reason", f"release {version}")
            boundary = (
                "No behavior, platform, artifact, or workflow result is inferred."
            )
        lines.append(
            "| "
            + " | ".join(
                (
                    f"[`{version}`]({tag_url})",
                    coverage,
                    evidence,
                    boundary,
                )
            )
            + " |"
        )

    observed = [release for release in releases if release.get("status") == "observed"]
    lines.extend(
        [
            "",
            "## Observed provenance",
            "",
        ]
    )
    for release in observed:
        version = required_string(release, "version", "observed release")
        lines.extend(
            [
                f"### Python pdfplumber {version}",
                "",
                (
                    "- Reference commit: "
                    f"`{required_string(release, 'target_commit', f'release {version}')}`."
                ),
                (
                    "- Candidate revision: "
                    f"`{required_string(release, 'subject_revision', f'release {version}')}`."
                ),
                (
                    "- Scorecard SHA-256: "
                    f"`{required_string(release, 'scorecard_sha256', f'release {version}')}`."
                ),
                "",
            ]
        )
    lines.extend(
        [
            (
                "Counts are retained evidence, not a release-level success metric. "
                "Exact observations do not cancel unsupported behavior, failures, "
                "or untested cells."
            ),
            "",
        ]
    )
    return "\n".join(lines)


def _required_mapping(
    data: Mapping[str, object],
    key: str,
    context: str,
) -> Mapping[str, object]:
    value = data.get(key)
    if not isinstance(value, dict):
        raise PythonReleaseMatrixError(f"{context} needs a {key} object")
    return value


def _required_mapping_sequence(
    data: Mapping[str, object],
    key: str,
    context: str,
) -> tuple[Mapping[str, object], ...]:
    value = data.get(key)
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise PythonReleaseMatrixError(f"{context} needs a {key} array")
    items: list[Mapping[str, object]] = []
    for position, item in enumerate(value):
        if not isinstance(item, dict):
            raise PythonReleaseMatrixError(
                f"{context} {key} entry {position} is invalid"
            )
        items.append(item)
    return tuple(items)


def main() -> int:
    arguments = parse_args()
    try:
        source_path = arguments.source.resolve()
        source = load_toml(source_path)
        destination = repository_path(source.get("output"), "matrix output")
        subject_version = required_string(source, "subject_version", "matrix source")
        if destination != (
            REPO_ROOT
            / "docs"
            / "compatibility"
            / f"python-release-matrix-v{subject_version}.md"
        ):
            raise PythonReleaseMatrixError(
                "matrix output must be versioned under docs/compatibility"
            )
        rendered = render(build(source))
        if arguments.check:
            try:
                committed = destination.read_text(encoding="utf-8")
            except OSError as error:
                raise PythonReleaseMatrixError(
                    f"cannot read committed matrix {destination}: {error}"
                ) from error
            if committed != rendered:
                raise PythonReleaseMatrixError(
                    f"Python release matrix is stale: {destination.relative_to(REPO_ROOT)}"
                )
            print(
                "Python release compatibility matrix is current: "
                f"{destination.relative_to(REPO_ROOT)}"
            )
            return 0

        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(rendered, encoding="utf-8")
        print(
            f"Wrote Python release compatibility matrix: {destination.relative_to(REPO_ROOT)}"
        )
        return 0
    except (
        OSError,
        compatibility_scorecard.ScorecardError,
        corpus_index.CorpusIndexError,
        PythonReleaseMatrixError,
        release_version.ReleaseVersionError,
    ) as error:
        print(f"Python release matrix failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
