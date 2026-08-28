"""Retained release-candidate benchmark and compatibility history (SCORE-014)."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, replace
from datetime import datetime, timezone
from pathlib import Path

import tomllib

from compat.harness import (
    benchmark_provenance,
    benchmark_results,
    compatibility_scorecard,
    corpus_index,
)

REPOSITORY = "https://github.com/developer0hye/pdfplumber-rs"
SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
CANDIDATE_PATTERN = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
RELEASE_LINE_PATTERN = re.compile(r"^[0-9]+\.[0-9]+$")
URL_PREFIXES = (
    f"{REPOSITORY}/actions/runs/",
    f"{REPOSITORY}/releases/tag/",
)
SUMMARY_DIMENSIONS = (
    "by_api",
    "by_option",
    "by_fixture_class",
    "by_page",
    "by_platform",
    "by_artifact_type",
)


class ScorecardHistoryError(ValueError):
    """A candidate result cannot be retained without losing evidence."""


@dataclass(frozen=True)
class HistoryPolicy:
    """Validated release-line history and publication policy."""

    schema_version: int
    identifier: str
    release_line: str
    runner: str
    artifact_prefix: str
    artifact_retention_days: int
    release_version: str | None = None
    history_path: Path | None = None
    report_path: Path | None = None
    workflow_source_path: Path | None = None
    corpus: corpus_index.CorpusIndex | None = None
    corpus_sha256: str | None = None
    benchmark_plan: benchmark_results.PublicationPlan | None = None


@dataclass(frozen=True)
class AssetBundle:
    """One deterministic candidate bundle and its checksum manifest."""

    benchmark_path: Path
    compatibility_path: Path
    workflow_path: Path
    history_path: Path
    report_path: Path
    checksums_path: Path

    @property
    def data_paths(self) -> tuple[Path, ...]:
        """Return the checksum-covered assets in stable order."""

        return (
            self.benchmark_path,
            self.compatibility_path,
            self.workflow_path,
            self.history_path,
            self.report_path,
        )

    @property
    def names(self) -> tuple[str, ...]:
        """Return every emitted asset name, including the checksum file."""

        return tuple(path.name for path in (*self.data_paths, self.checksums_path))


def validate_policy(source: Mapping[str, object]) -> HistoryPolicy:
    """Validate a parsed history-policy manifest."""

    schema = _required_mapping(source, "schema", "manifest")
    if schema.get("version") != 1:
        raise ScorecardHistoryError("schema.version must be 1")
    policy = _required_mapping(source, "policy", "manifest")
    identifier = _required_string(policy, "id", "policy")
    release_line = _required_string(policy, "release_line", "policy")
    if RELEASE_LINE_PATTERN.fullmatch(release_line) is None:
        raise ScorecardHistoryError("policy release_line must be major.minor")
    runner = _required_string(policy, "runner", "policy")
    if runner != "macos-14":
        raise ScorecardHistoryError("release-candidate runner must be macos-14")
    artifact_prefix = _required_string(policy, "artifact_prefix", "policy")
    if CANDIDATE_PATTERN.fullmatch(artifact_prefix) is None:
        raise ScorecardHistoryError("policy artifact_prefix is not asset-safe")
    retention = policy.get("artifact_retention_days")
    if retention != 90:
        raise ScorecardHistoryError(
            "candidate workflow artifacts must be retained 90 days"
        )
    release_version = _required_string(policy, "release_version", "policy")
    if ".".join(release_version.split(".")[:2]) != release_line:
        raise ScorecardHistoryError("release_version is outside the release line")
    return HistoryPolicy(
        schema_version=1,
        identifier=identifier,
        release_line=release_line,
        runner=runner,
        artifact_prefix=artifact_prefix,
        artifact_retention_days=retention,
        release_version=release_version,
    )


def audit_repository(repo_root: Path, manifest_path: Path) -> HistoryPolicy:
    """Validate the policy, inherited benchmark inputs, and committed history."""

    source = _load_toml(manifest_path)
    policy = validate_policy(source)
    paths = _required_mapping(source, "paths", "manifest")
    benchmark = _required_mapping(source, "benchmark", "manifest")

    history_path = _repository_path(
        repo_root, _required_string(paths, "history", "paths")
    )
    report_path = _repository_path(
        repo_root, _required_string(paths, "report", "paths")
    )
    workflow_source_path = _repository_path(
        repo_root, _required_string(paths, "workflow_definitions", "paths")
    )
    corpus_path = _repository_path(
        repo_root, _required_string(paths, "corpus", "paths")
    )
    publication_path = _repository_path(
        repo_root, _required_string(benchmark, "publication", "benchmark")
    )
    provenance_path = _repository_path(
        repo_root, _required_string(benchmark, "provenance", "benchmark")
    )
    scenarios_path = _repository_path(
        repo_root, _required_string(benchmark, "scenarios", "benchmark")
    )
    competitors_path = _repository_path(
        repo_root, _required_string(benchmark, "competitors", "benchmark")
    )
    benchmark_corpus_path = _repository_path(
        repo_root, _required_string(benchmark, "corpus", "benchmark")
    )
    equivalence_path = _repository_path(
        repo_root, _required_string(benchmark, "equivalence", "benchmark")
    )
    registry_path = _repository_path(
        repo_root, _required_string(benchmark, "registry", "benchmark")
    )
    benchmark_plan = benchmark_results.audit_repository(
        repo_root,
        publication_path,
        provenance_path,
        scenarios_path,
        competitors_path,
        benchmark_corpus_path,
        equivalence_path,
        registry_path,
    )
    if benchmark_plan.release != policy.release_version:
        raise ScorecardHistoryError(
            "candidate history and benchmark publication releases differ"
        )

    corpus = corpus_index.load_index(corpus_path)
    corpus_sha256 = _file_sha256(corpus_path)
    audited = replace(
        policy,
        history_path=history_path,
        report_path=report_path,
        workflow_source_path=workflow_source_path,
        corpus=corpus,
        corpus_sha256=corpus_sha256,
        benchmark_plan=benchmark_plan,
    )
    history = load_history(history_path)
    validate_history(audited, history)
    if history_path.read_text(encoding="utf-8") != render_history(history):
        raise ScorecardHistoryError("committed candidate history is not canonical")
    if report_path.read_text(encoding="utf-8") != render_report(audited, history):
        raise ScorecardHistoryError("committed candidate history report is stale")
    return audited


def empty_history(policy: HistoryPolicy) -> dict[str, object]:
    """Return an empty, explicit history for a release line."""

    return {
        "schema_version": 1,
        "policy_id": policy.identifier,
        "release_line": policy.release_line,
        "status_vocabulary": copy.deepcopy(compatibility_scorecard.STATUS_VOCABULARY),
        "runs": [],
    }


def load_history(path: Path) -> dict[str, object]:
    """Read one committed or downloaded history artifact."""

    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ScorecardHistoryError(f"cannot read candidate history: {path}") from error
    if not isinstance(value, dict):
        raise ScorecardHistoryError("candidate history root must be an object")
    return value


def build_entry(
    policy: HistoryPolicy,
    *,
    candidate_id: str,
    source_revision: str,
    run_url: str,
    benchmark_run: Mapping[str, object],
    compatibility: Mapping[str, object],
    workflow_report: str,
) -> dict[str, object]:
    """Build one lossless compact history entry from both candidate results."""

    _validate_candidate_identity(candidate_id, source_revision, run_url)
    if not isinstance(workflow_report, str) or not workflow_report.strip():
        raise ScorecardHistoryError("workflow scorecard report must be non-empty")
    _reject_percentage_fields(benchmark_run, "benchmark run")
    _reject_percentage_fields(compatibility, "compatibility scorecard")

    metadata = _required_mapping(benchmark_run, "run_metadata", "benchmark run")
    source = _required_mapping(metadata, "source", "benchmark metadata")
    if source.get("revision") != source_revision:
        raise ScorecardHistoryError(
            "benchmark source revision differs from the candidate revision"
        )
    if source.get("repository") != REPOSITORY:
        raise ScorecardHistoryError("benchmark source repository is not canonical")
    if source.get("working_tree_clean") is not True:
        raise ScorecardHistoryError("benchmark source worktree was not clean")
    recorded_at_utc = _required_string(metadata, "recorded_at_utc", "metadata")
    _parse_timestamp(recorded_at_utc)

    records = _required_object_sequence(benchmark_run, "records", "benchmark run")
    decisions = _required_object_sequence(
        benchmark_run, "preflight_decisions", "benchmark run"
    )
    timings = _required_object_sequence(
        benchmark_run, "scenario_timings", "benchmark run"
    )
    summaries = _required_object_sequence(
        benchmark_run, "statistical_summaries", "benchmark run"
    )
    if not records or not decisions or not timings or not summaries:
        raise ScorecardHistoryError("benchmark result collections must be non-empty")
    try:
        benchmark_provenance.validate_run_metadata(metadata, repetitions=5)
        expected_summaries = benchmark_provenance.summarize_samples(
            timings, repetitions=5
        )
    except benchmark_provenance.BenchmarkProvenanceError as error:
        raise ScorecardHistoryError(str(error)) from error
    if _canonical_json(expected_summaries) != _canonical_json(summaries):
        raise ScorecardHistoryError(
            "benchmark statistical summaries do not match the raw samples"
        )
    if policy.benchmark_plan is not None:
        try:
            benchmark_results.publish_run(
                policy.benchmark_plan,
                benchmark_run,
                release_tag=policy.benchmark_plan.release_tag,
                source_revision=source_revision,
            )
        except benchmark_results.BenchmarkResultError as error:
            raise ScorecardHistoryError(str(error)) from error

    subject = _required_mapping(compatibility, "subject", "compatibility scorecard")
    if subject.get("revision") != source_revision:
        raise ScorecardHistoryError(
            "compatibility subject revision differs from the candidate revision"
        )
    if subject.get("project") != "pdfplumber-rs":
        raise ScorecardHistoryError(
            "compatibility subject project is not pdfplumber-rs"
        )
    if (
        policy.release_version is not None
        and subject.get("version") != policy.release_version
    ):
        raise ScorecardHistoryError(
            "compatibility subject version differs from the release policy"
        )
    if policy.corpus is not None and policy.corpus_sha256 is not None:
        try:
            compatibility_scorecard.validate(
                compatibility,
                corpus=policy.corpus,
                corpus_sha256=policy.corpus_sha256,
            )
        except compatibility_scorecard.ScorecardError as error:
            raise ScorecardHistoryError(str(error)) from error
    vocabulary = _required_mapping(
        compatibility, "status_vocabulary", "compatibility scorecard"
    )
    if set(vocabulary) != set(compatibility_scorecard.STATUSES):
        raise ScorecardHistoryError("compatibility status vocabulary is incomplete")
    observations = _required_object_sequence(
        compatibility, "observations", "compatibility scorecard"
    )
    summary = _required_mapping(compatibility, "summary", "compatibility scorecard")
    status_counts = _validate_status_counts(
        summary.get("status_counts"), "compatibility status counts"
    )
    if sum(status_counts.values()) != len(observations):
        raise ScorecardHistoryError(
            "compatibility status counts do not cover every observation"
        )
    for dimension in SUMMARY_DIMENSIONS:
        values = summary.get(dimension)
        if not isinstance(values, list):
            raise ScorecardHistoryError(
                f"compatibility summary has no {dimension} array"
            )

    benchmark_bytes = _json_bytes(benchmark_run)
    compatibility_bytes = compatibility_scorecard.render(compatibility).encode("utf-8")
    workflow_bytes = workflow_report.encode("utf-8")
    names = asset_names(policy, candidate_id)
    status_counter = Counter()
    for record in records:
        outcome = _required_mapping(record, "outcome", "benchmark record")
        status_counter[_required_string(outcome, "status", "benchmark outcome")] += 1
    decision_counter = Counter(
        "exact" if decision.get("eligible_for_timing") is True else "rejected"
        for decision in decisions
    )

    return {
        "candidate_id": candidate_id,
        "recorded_at_utc": recorded_at_utc,
        "source_revision": source_revision,
        "runner": policy.runner,
        "run_url": run_url,
        "previous_entry_sha256": None,
        "benchmark": {
            "asset": names["benchmark"],
            "raw_sha256": _bytes_sha256(benchmark_bytes),
            "semantic_record_count": len(records),
            "status_counts": dict(sorted(status_counter.items())),
            "decision_counts": dict(sorted(decision_counter.items())),
            "raw_sample_count": len(timings),
            "timed_group_count": len(summaries),
            "statistical_summaries": copy.deepcopy(summaries),
        },
        "compatibility": {
            "asset": names["compatibility"],
            "machine_sha256": _bytes_sha256(compatibility_bytes),
            "workflow_asset": names["workflows"],
            "workflow_sha256": _bytes_sha256(workflow_bytes),
            "observation_count": len(observations),
            "status_counts": copy.deepcopy(status_counts),
            "summary": copy.deepcopy(dict(summary)),
        },
    }


def append_entry(
    policy: HistoryPolicy,
    history: Mapping[str, object],
    entry: Mapping[str, object],
) -> dict[str, object]:
    """Append one candidate while retaining and chaining every prior run."""

    validate_history(policy, history)
    existing_runs = _required_object_sequence(history, "runs", "history")
    candidate_id = _required_string(entry, "candidate_id", "history entry")
    if any(run.get("candidate_id") == candidate_id for run in existing_runs):
        raise ScorecardHistoryError(f"duplicate candidate ID: {candidate_id}")
    recorded_at = _required_string(entry, "recorded_at_utc", "history entry")
    _parse_timestamp(recorded_at)
    if existing_runs:
        previous_time = _required_string(
            existing_runs[-1], "recorded_at_utc", "previous history entry"
        )
        if recorded_at <= previous_time:
            raise ScorecardHistoryError(
                "candidate history entries must be chronological"
            )

    appended = copy.deepcopy(dict(history))
    appended_runs = _required_object_sequence(appended, "runs", "history")
    copied_entry = copy.deepcopy(dict(entry))
    copied_entry["previous_entry_sha256"] = (
        entry_sha256(appended_runs[-1]) if appended_runs else None
    )
    appended_runs.append(copied_entry)
    validate_history(policy, appended)
    return appended


def validate_history(policy: HistoryPolicy, history: Mapping[str, object]) -> None:
    """Validate history identity, chronology, counts, and the digest chain."""

    if history.get("schema_version") != 1:
        raise ScorecardHistoryError("candidate history schema_version must be 1")
    if history.get("policy_id") != policy.identifier:
        raise ScorecardHistoryError("candidate history names the wrong policy")
    if history.get("release_line") != policy.release_line:
        raise ScorecardHistoryError("candidate history names the wrong release line")
    if history.get("status_vocabulary") != compatibility_scorecard.STATUS_VOCABULARY:
        raise ScorecardHistoryError("candidate history status vocabulary has drifted")
    _reject_percentage_fields(history, "candidate history")
    runs = _required_object_sequence(history, "runs", "history")
    seen_ids: set[str] = set()
    previous: Mapping[str, object] | None = None
    previous_time: str | None = None
    for entry in runs:
        _validate_retained_entry(policy, entry)
        candidate_id = _required_string(entry, "candidate_id", "history entry")
        if candidate_id in seen_ids:
            raise ScorecardHistoryError(f"duplicate candidate ID: {candidate_id}")
        seen_ids.add(candidate_id)
        recorded_at = _required_string(entry, "recorded_at_utc", "history entry")
        _parse_timestamp(recorded_at)
        if previous_time is not None and recorded_at <= previous_time:
            raise ScorecardHistoryError(
                "candidate history entries must be chronological"
            )
        expected_previous = entry_sha256(previous) if previous is not None else None
        if entry.get("previous_entry_sha256") != expected_previous:
            raise ScorecardHistoryError(
                f"candidate {candidate_id} breaks the history digest chain"
            )
        previous = entry
        previous_time = recorded_at


def entry_sha256(entry: Mapping[str, object]) -> str:
    """Return the canonical digest of one complete retained entry."""

    return hashlib.sha256(_canonical_json(entry).encode("utf-8")).hexdigest()


def asset_names(policy: HistoryPolicy, candidate_id: str) -> dict[str, str]:
    """Return exact candidate-and-release-line-bound asset names."""

    if CANDIDATE_PATTERN.fullmatch(candidate_id) is None:
        raise ScorecardHistoryError(f"invalid candidate ID: {candidate_id}")
    stem = f"{policy.artifact_prefix}-{candidate_id}"
    return {
        "benchmark": f"{stem}-benchmark.json",
        "compatibility": f"{stem}-compatibility.json",
        "workflows": f"{stem}-workflows.md",
        "history": f"{stem}-history.json",
        "report": f"{stem}-history.md",
        "checksums": f"{stem}.sha256",
    }


def write_assets(
    policy: HistoryPolicy,
    output_directory: Path,
    *,
    entry: Mapping[str, object],
    history: Mapping[str, object],
    benchmark_run: Mapping[str, object],
    compatibility: Mapping[str, object],
    workflow_report: str,
) -> AssetBundle:
    """Write the current raw results, cumulative history, report, and checksums."""

    validate_history(policy, history)
    runs = _required_object_sequence(history, "runs", "history")
    if not runs or _canonical_json(runs[-1]) != _canonical_json(entry):
        raise ScorecardHistoryError("asset entry is not the latest history entry")
    names = asset_names(
        policy, _required_string(entry, "candidate_id", "history entry")
    )
    output_directory.mkdir(parents=True, exist_ok=True)
    bundle = AssetBundle(
        benchmark_path=output_directory / names["benchmark"],
        compatibility_path=output_directory / names["compatibility"],
        workflow_path=output_directory / names["workflows"],
        history_path=output_directory / names["history"],
        report_path=output_directory / names["report"],
        checksums_path=output_directory / names["checksums"],
    )
    contents = (
        (bundle.benchmark_path, _json_bytes(benchmark_run)),
        (
            bundle.compatibility_path,
            compatibility_scorecard.render(compatibility).encode("utf-8"),
        ),
        (bundle.workflow_path, workflow_report.encode("utf-8")),
        (bundle.history_path, render_history(history).encode("utf-8")),
        (bundle.report_path, render_report(policy, history).encode("utf-8")),
    )
    benchmark = _required_mapping(entry, "benchmark", "history entry")
    compatibility_entry = _required_mapping(entry, "compatibility", "history entry")
    expected_digests = {
        bundle.benchmark_path.name: benchmark.get("raw_sha256"),
        bundle.compatibility_path.name: compatibility_entry.get("machine_sha256"),
        bundle.workflow_path.name: compatibility_entry.get("workflow_sha256"),
    }
    for path, content in contents:
        expected = expected_digests.get(path.name)
        if expected is not None and _bytes_sha256(content) != expected:
            raise ScorecardHistoryError(
                f"asset {path.name} differs from its retained digest"
            )
        path.write_bytes(content)
    checksums = "".join(
        f"{_file_sha256(path)}  {path.name}\n" for path in bundle.data_paths
    )
    bundle.checksums_path.write_text(checksums, encoding="utf-8")
    return bundle


def render_history(history: Mapping[str, object]) -> str:
    """Render canonical pretty JSON for version control and release assets."""

    return json.dumps(history, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def render_report(policy: HistoryPolicy, history: Mapping[str, object]) -> str:
    """Render an outcome-count trend report without a success percentage."""

    validate_history(policy, history)
    runs = _required_object_sequence(history, "runs", "history")
    lines = [
        f"# Release-candidate scorecard history — {policy.release_line}.x",
        "",
        (
            "Every recorded candidate retains its competitor benchmark groups and all six "
            "compatibility outcomes. Rows are chronological observations, not rankings or "
            "broad performance claims."
        ),
        "",
    ]
    if not runs:
        lines.extend(
            [
                "No release-candidate run has been recorded for this release line yet.",
                "",
                (
                    "The release workflow must generate and retain the first row before "
                    "this history can be used for a trend comparison."
                ),
                "",
            ]
        )
        return "\n".join(lines)

    lines.extend(
        [
            "| Candidate | Recorded | Source | Semantic records | Exact decisions | Rejected decisions | Timed groups | Exact | Approved delta | Unsupported | Reference failure | Candidate failure | Not tested |",
            "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for entry in runs:
        benchmark = _required_mapping(entry, "benchmark", "history entry")
        decisions = _required_mapping(benchmark, "decision_counts", "benchmark")
        compatibility = _required_mapping(entry, "compatibility", "history entry")
        counts = _validate_status_counts(
            compatibility.get("status_counts"), "compatibility status counts"
        )
        candidate_id = _required_string(entry, "candidate_id", "history entry")
        run_url = _required_string(entry, "run_url", "history entry")
        source_revision = _required_string(entry, "source_revision", "history entry")
        lines.append(
            "| "
            + " | ".join(
                (
                    f"[{candidate_id}]({run_url})",
                    _required_string(entry, "recorded_at_utc", "history entry"),
                    f"`{source_revision[:12]}`",
                    str(benchmark.get("semantic_record_count")),
                    str(decisions.get("exact", 0)),
                    str(decisions.get("rejected", 0)),
                    str(benchmark.get("timed_group_count")),
                    str(counts["exact"]),
                    str(counts["approved_delta"]),
                    str(counts["unsupported"]),
                    str(counts["reference_failure"]),
                    str(counts["candidate_failure"]),
                    str(counts["not_tested"]),
                )
            )
            + " |"
        )
    lines.extend(
        [
            "",
            (
                "Each machine history entry also retains the complete benchmark "
                "statistical summaries and compatibility summaries by API, option, "
                "fixture class, page, platform, and artifact type. Follow the candidate "
                "link for its raw assets."
            ),
            "",
        ]
    )
    return "\n".join(lines)


def _validate_retained_entry(
    policy: HistoryPolicy, entry: Mapping[str, object]
) -> None:
    candidate_id = _required_string(entry, "candidate_id", "history entry")
    source_revision = _required_string(entry, "source_revision", "history entry")
    run_url = _required_string(entry, "run_url", "history entry")
    _validate_candidate_identity(candidate_id, source_revision, run_url)
    if entry.get("runner") != policy.runner:
        raise ScorecardHistoryError(f"candidate {candidate_id} used the wrong runner")
    previous = entry.get("previous_entry_sha256")
    if previous is not None and (
        not isinstance(previous, str) or SHA256_PATTERN.fullmatch(previous) is None
    ):
        raise ScorecardHistoryError(
            f"candidate {candidate_id} has an invalid previous digest"
        )
    benchmark = _required_mapping(entry, "benchmark", "history entry")
    compatibility = _required_mapping(entry, "compatibility", "history entry")
    names = asset_names(policy, candidate_id)
    for value, name, label in (
        (benchmark.get("asset"), names["benchmark"], "benchmark asset"),
        (
            compatibility.get("asset"),
            names["compatibility"],
            "compatibility asset",
        ),
        (
            compatibility.get("workflow_asset"),
            names["workflows"],
            "workflow asset",
        ),
    ):
        if value != name:
            raise ScorecardHistoryError(
                f"candidate {candidate_id} has the wrong {label}"
            )
    for value, label in (
        (benchmark.get("raw_sha256"), "benchmark digest"),
        (compatibility.get("machine_sha256"), "compatibility digest"),
        (compatibility.get("workflow_sha256"), "workflow digest"),
    ):
        if not isinstance(value, str) or SHA256_PATTERN.fullmatch(value) is None:
            raise ScorecardHistoryError(f"candidate {candidate_id} has invalid {label}")
    for key in (
        "semantic_record_count",
        "raw_sample_count",
        "timed_group_count",
    ):
        if not isinstance(benchmark.get(key), int) or benchmark[key] <= 0:
            raise ScorecardHistoryError(
                f"candidate {candidate_id} has invalid benchmark {key}"
            )
    summaries = benchmark.get("statistical_summaries")
    if (
        not isinstance(summaries, list)
        or len(summaries) != benchmark["timed_group_count"]
    ):
        raise ScorecardHistoryError(
            f"candidate {candidate_id} has incomplete statistical summaries"
        )
    if not isinstance(compatibility.get("observation_count"), int):
        raise ScorecardHistoryError(
            f"candidate {candidate_id} has invalid compatibility observation count"
        )
    counts = _validate_status_counts(
        compatibility.get("status_counts"), "compatibility status counts"
    )
    if sum(counts.values()) != compatibility["observation_count"]:
        raise ScorecardHistoryError(
            f"candidate {candidate_id} has incomplete compatibility counts"
        )
    summary = _required_mapping(compatibility, "summary", "compatibility")
    if summary.get("status_counts") != counts:
        raise ScorecardHistoryError(
            f"candidate {candidate_id} summary counts have drifted"
        )
    for dimension in SUMMARY_DIMENSIONS:
        if not isinstance(summary.get(dimension), list):
            raise ScorecardHistoryError(
                f"candidate {candidate_id} summary lacks {dimension}"
            )


def _validate_candidate_identity(
    candidate_id: str, source_revision: str, run_url: str
) -> None:
    if CANDIDATE_PATTERN.fullmatch(candidate_id) is None:
        raise ScorecardHistoryError(f"invalid candidate ID: {candidate_id}")
    if SHA_PATTERN.fullmatch(source_revision) is None:
        raise ScorecardHistoryError("candidate source revision must be a full Git SHA")
    if not any(run_url.startswith(prefix) for prefix in URL_PREFIXES):
        raise ScorecardHistoryError(
            "candidate run URL is outside the canonical repository"
        )


def _validate_status_counts(value: object, context: str) -> dict[str, int]:
    if not isinstance(value, dict) or set(value) != set(
        compatibility_scorecard.STATUSES
    ):
        raise ScorecardHistoryError(f"{context} do not cover the exact vocabulary")
    if any(not isinstance(count, int) or count < 0 for count in value.values()):
        raise ScorecardHistoryError(f"{context} must be non-negative integers")
    return dict(value)


def _parse_timestamp(value: str) -> datetime:
    try:
        parsed = datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=timezone.utc
        )
    except ValueError as error:
        raise ScorecardHistoryError(
            "recorded_at_utc must use YYYY-MM-DDTHH:MM:SSZ"
        ) from error
    return parsed


def _reject_percentage_fields(value: object, context: str) -> None:
    if isinstance(value, Mapping):
        for key, nested in value.items():
            if "percentage" in str(key).lower():
                raise ScorecardHistoryError(f"{context} contains a percentage field")
            _reject_percentage_fields(nested, context)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        for nested in value:
            _reject_percentage_fields(nested, context)


def _load_toml(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as source_file:
            value = tomllib.load(source_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ScorecardHistoryError(f"cannot read history policy: {path}") from error
    if not isinstance(value, dict):
        raise ScorecardHistoryError("history policy root must be a table")
    return value


def _repository_path(repo_root: Path, value: str) -> Path:
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise ScorecardHistoryError(f"unsafe repository path: {value}")
    return repo_root / relative


def _required_mapping(
    source: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise ScorecardHistoryError(f"{context} has no {key} object")
    return value


def _required_string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value:
        raise ScorecardHistoryError(f"{context} has no non-empty {key}")
    return value


def _required_object_sequence(
    source: Mapping[str, object], key: str, context: str
) -> list[dict[str, object]]:
    value = source.get(key)
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        raise ScorecardHistoryError(f"{context} has no object array {key}")
    return value


def _canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _bytes_sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()
