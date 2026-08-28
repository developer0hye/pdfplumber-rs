"""Versioned benchmark release assets and human report (SCORE-008)."""

from __future__ import annotations

import hashlib
import json
import re
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_provenance

REPOSITORY = "https://github.com/developer0hye/pdfplumber-rs"
ID_PATTERN = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
ASSET_PATTERN = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
REFERENCE_IMPLEMENTATION = "pdfplumber-python"
LOCAL_RUN_KEYS = {
    "schema_version",
    "publication_status",
    "run_metadata",
    "records",
    "preflight_decisions",
    "scenario_timings",
    "statistical_summaries",
}


class BenchmarkResultError(ValueError):
    """Raised when a benchmark result cannot become a release artifact."""


@dataclass(frozen=True)
class PublicationPlan:
    """Validated versioned release identity and asset names."""

    id: str
    release: str
    provenance_suite_id: str
    release_tag: str
    source_policy: str
    runner: str
    raw_asset: str
    report_asset: str
    checksums_asset: str
    provenance_plan: benchmark_provenance.ProvenancePlan

    @property
    def raw_url(self) -> str:
        return self.asset_url(self.raw_asset)

    @property
    def report_url(self) -> str:
        return self.asset_url(self.report_asset)

    @property
    def checksums_url(self) -> str:
        return self.asset_url(self.checksums_asset)

    def asset_url(self, asset: str) -> str:
        return f"{REPOSITORY}/releases/download/{self.release_tag}/{asset}"


@dataclass(frozen=True)
class ReleaseAssets:
    """Paths, bytes, and digests for one deterministic release bundle."""

    raw_path: Path
    report_path: Path
    checksums_path: Path
    raw_bytes: bytes
    report_bytes: bytes
    checksums_bytes: bytes
    raw_sha256: str
    report_sha256: str


def audit_repository(
    repo_root: Path,
    publication_path: Path,
    provenance_path: Path,
    scenario_path: Path,
    competitor_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> PublicationPlan:
    """Validate the publication manifest and every inherited benchmark input."""

    provenance_plan = benchmark_provenance.audit_repository(
        repo_root,
        provenance_path,
        scenario_path,
        competitor_path,
        corpus_path,
        policy_path,
        registry_path,
    )
    try:
        with publication_path.open("rb") as publication_file:
            source = tomllib.load(publication_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkResultError(
            f"cannot read publication manifest: {publication_path}"
        ) from error
    return validate_plan(source, provenance_plan)


def validate_plan(
    source: Mapping[str, object],
    provenance_plan: benchmark_provenance.ProvenancePlan,
) -> PublicationPlan:
    """Validate one parsed publication manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkResultError("schema.version must be 1")
    publication = _required_mapping(source, "publication", "manifest")
    publication_id = _required_string(publication, "id", "publication")
    if not ID_PATTERN.fullmatch(publication_id):
        raise BenchmarkResultError(f"invalid publication id: {publication_id}")
    release = _required_string(publication, "release", "publication")
    if release != provenance_plan.release:
        raise BenchmarkResultError("publication and provenance releases differ")
    provenance_suite_id = _required_string(
        publication, "provenance_suite_id", "publication"
    )
    if provenance_suite_id != provenance_plan.id:
        raise BenchmarkResultError("publication names the wrong provenance suite")
    release_tag = _required_string(publication, "release_tag", "publication")
    if release_tag != f"benchmark-results-v{release}":
        raise BenchmarkResultError("publication release tag is not canonical")
    source_policy = _required_string(publication, "source_policy", "publication")
    if source_policy != "exact-tag-target":
        raise BenchmarkResultError("publication source policy must be exact-tag-target")
    runner = _required_string(publication, "runner", "publication")
    if runner != "macos-14":
        raise BenchmarkResultError("publication runner must be macos-14")

    expected_assets = {
        "raw_asset": f"pdfplumber-rs-benchmark-results-v{release}.json",
        "report_asset": f"pdfplumber-rs-benchmark-results-v{release}.md",
        "checksums_asset": f"pdfplumber-rs-benchmark-results-v{release}.sha256",
    }
    assets = {
        field: _required_string(publication, field, "publication")
        for field in expected_assets
    }
    if assets != expected_assets:
        raise BenchmarkResultError(
            f"publication assets must be exactly {expected_assets}"
        )
    if len(set(assets.values())) != len(assets):
        raise BenchmarkResultError("publication asset names must be unique")
    for asset in assets.values():
        if not ASSET_PATTERN.fullmatch(asset) or "/" in asset:
            raise BenchmarkResultError(f"invalid publication asset name: {asset}")

    return PublicationPlan(
        id=publication_id,
        release=release,
        provenance_suite_id=provenance_suite_id,
        release_tag=release_tag,
        source_policy=source_policy,
        runner=runner,
        raw_asset=assets["raw_asset"],
        report_asset=assets["report_asset"],
        checksums_asset=assets["checksums_asset"],
        provenance_plan=provenance_plan,
    )


def publish_run(
    plan: PublicationPlan,
    local_run: Mapping[str, object],
    *,
    release_tag: str,
    source_revision: str,
) -> dict[str, object]:
    """Promote a complete exact-tag local run without dropping any evidence."""

    if release_tag != plan.release_tag:
        raise BenchmarkResultError(
            f"release tag {release_tag!r} does not match {plan.release_tag!r}"
        )
    if not SHA_PATTERN.fullmatch(source_revision):
        raise BenchmarkResultError("source revision must be a full Git SHA")
    if set(local_run) != LOCAL_RUN_KEYS:
        raise BenchmarkResultError(
            f"local run keys are {sorted(local_run)}, expected {sorted(LOCAL_RUN_KEYS)}"
        )
    if local_run.get("schema_version") != 1:
        raise BenchmarkResultError("local run schema_version must be 1")
    if local_run.get("publication_status") != "local-unpublished":
        raise BenchmarkResultError("input must be a local-unpublished run")

    metadata = _required_mapping(local_run, "run_metadata", "local run")
    benchmark_provenance.validate_run_metadata(
        metadata,
        repetitions=plan.provenance_plan.repetitions,
    )
    source = _required_mapping(metadata, "source", "run metadata")
    recorded_revision = _required_string(source, "revision", "source")
    if recorded_revision != source_revision:
        raise BenchmarkResultError(
            "recorded source revision does not match the exact release tag target"
        )

    records = _required_object_array(local_run, "records")
    decisions = _required_object_array(local_run, "preflight_decisions")
    timings = _required_object_array(local_run, "scenario_timings")
    summaries = _required_object_array(local_run, "statistical_summaries")
    if not records or not decisions or not timings or not summaries:
        raise BenchmarkResultError("release run collections must be non-empty")

    expected_summaries = benchmark_provenance.summarize_samples(
        timings,
        repetitions=plan.provenance_plan.repetitions,
    )
    if _canonical_json(expected_summaries) != _canonical_json(summaries):
        raise BenchmarkResultError(
            "statistical summaries do not match the complete raw samples"
        )
    _validate_timing_eligibility(plan, decisions, timings)

    published = {key: local_run[key] for key in LOCAL_RUN_KEYS}
    published["publication_status"] = "release-artifact"
    published["publication"] = {
        "repository": REPOSITORY,
        "release": plan.release,
        "release_tag": plan.release_tag,
        "source_policy": plan.source_policy,
        "source_revision": source_revision,
        "raw_asset": plan.raw_asset,
        "raw_url": plan.raw_url,
        "report_asset": plan.report_asset,
        "report_url": plan.report_url,
        "checksums_asset": plan.checksums_asset,
        "checksums_url": plan.checksums_url,
    }
    return published


def write_release_assets(
    plan: PublicationPlan,
    local_run: Mapping[str, object],
    output_directory: Path,
    *,
    release_tag: str,
    source_revision: str,
) -> ReleaseAssets:
    """Write deterministic raw JSON, human Markdown, and checksum assets."""

    published = publish_run(
        plan,
        local_run,
        release_tag=release_tag,
        source_revision=source_revision,
    )
    raw_bytes = (_canonical_json_pretty(published) + "\n").encode("utf-8")
    raw_sha256 = hashlib.sha256(raw_bytes).hexdigest()
    report_bytes = render_human_report(plan, published, raw_sha256).encode("utf-8")
    report_sha256 = hashlib.sha256(report_bytes).hexdigest()
    checksums_bytes = (
        f"{raw_sha256}  {plan.raw_asset}\n"
        f"{report_sha256}  {plan.report_asset}\n"
    ).encode()

    output_directory.mkdir(parents=True, exist_ok=True)
    raw_path = output_directory / plan.raw_asset
    report_path = output_directory / plan.report_asset
    checksums_path = output_directory / plan.checksums_asset
    raw_path.write_bytes(raw_bytes)
    report_path.write_bytes(report_bytes)
    checksums_path.write_bytes(checksums_bytes)
    return ReleaseAssets(
        raw_path=raw_path,
        report_path=report_path,
        checksums_path=checksums_path,
        raw_bytes=raw_bytes,
        report_bytes=report_bytes,
        checksums_bytes=checksums_bytes,
        raw_sha256=raw_sha256,
        report_sha256=report_sha256,
    )


def render_human_report(
    plan: PublicationPlan,
    published: Mapping[str, object],
    raw_sha256: str,
) -> str:
    """Render a concise projection of one complete machine-readable run."""

    metadata = _required_mapping(published, "run_metadata", "published run")
    source = _required_mapping(metadata, "source", "run metadata")
    host = _required_mapping(metadata, "host", "run metadata")
    timings = _required_object_array(published, "scenario_timings")
    summaries = _required_object_array(published, "statistical_summaries")
    decisions = _required_object_array(published, "preflight_decisions")
    records = _required_object_array(published, "records")
    rejected = [decision for decision in decisions if not decision["eligible_for_timing"]]
    eligible = [decision for decision in decisions if decision["eligible_for_timing"]]
    outcomes = Counter(
        _required_string(_required_mapping(record, "outcome", "record"), "status", "outcome")
        for record in records
    )
    toolchains = _required_object_array(metadata, "toolchains")
    locks = _required_object_array(metadata, "dependency_locks")

    rejection_rows = []
    for decision in sorted(
        rejected,
        key=lambda item: (
            str(item["scenario_id"]),
            str(item["case_id"]),
            str(item["implementation_id"]),
        ),
    ):
        reasons = decision.get("reasons")
        if not isinstance(reasons, list) or not reasons:
            raise BenchmarkResultError("rejected comparison has no reason")
        rejection_rows.append(
            "| "
            + " | ".join(
                (
                    f"`{decision['case_id']}`",
                    f"`{decision['implementation_id']}`",
                    _markdown_text("; ".join(str(reason) for reason in reasons)),
                )
            )
            + " |"
        )

    timing_rows = []
    for summary in sorted(
        summaries,
        key=lambda item: (
            str(item["scenario_id"]),
            str(item["case_id"]),
            str(_required_mapping(item, "implementation", "summary")["id"]),
        ),
    ):
        implementation = _required_mapping(summary, "implementation", "summary")
        timing_rows.append(
            "| "
            + " | ".join(
                (
                    f"`{summary['case_id']}`",
                    f"`{implementation['id']}`",
                    str(summary["sample_size"]),
                    _milliseconds(summary["median_wall_time_ns"]),
                    _milliseconds(summary["arithmetic_mean_wall_time_ns"]),
                    f"{float(summary['relative_standard_deviation']) * 100:.2f}%",
                )
            )
            + " |"
        )

    tool_rows = [
        f"| `{tool['id']}` | {_markdown_text(str(tool['version']))} |"
        for tool in toolchains
    ]
    lock_rows = [
        f"| `{lock['role']}` | `{lock['path']}` | `{lock['sha256']}` |"
        for lock in locks
    ]
    rejection_count = len(rejected)
    rejection_label = (
        "rejected comparison" if rejection_count == 1 else "rejected comparisons"
    )
    outcome_text = ", ".join(
        f"{count} `{status}`" for status, count in sorted(outcomes.items())
    )
    return "\n".join(
        (
            f"# Benchmark Results v{plan.release}",
            "",
            f"Release tag: `{plan.release_tag}`  ",
            f"Exact source: `{source['revision']}`  ",
            f"Recorded at: `{metadata['recorded_at_utc']}`  ",
            f"Complete raw result: [{plan.raw_asset}]({plan.raw_url})  ",
            f"Raw-result SHA-256: `{raw_sha256}`  ",
            f"Checksums: [{plan.checksums_asset}]({plan.checksums_url})",
            "",
            "These are descriptive observations from one recorded host. Semantic output equivalence is checked before every retained timing. Rejected comparisons stay visible and untimed. The tables do not define a regression threshold, confidence interval, product ranking, or broad performance claim.",
            "",
            "## Coverage",
            "",
            f"The run retains {len(records)} semantic records ({outcome_text}), {len(decisions)} non-reference preflight decisions ({len(eligible)} exact and {rejection_count} rejected), {len(timings)} raw samples, and {len(summaries)} timed groups. Every timed group contains repetitions 1 through {plan.provenance_plan.repetitions}.",
            "",
            f"There are **{rejection_count} {rejection_label}**; none contributes a timing row.",
            "",
            "| Case | Implementation | Reason |",
            "|---|---|---|",
            *rejection_rows,
            "",
            "## Host and tools",
            "",
            f"Host: `{host['operating_system']} {host['operating_system_release']}` on `{host['architecture']}`; Central Processing Unit `{host['cpu_model']}` with {host['logical_cpu_count']} logical processors; {host['physical_memory_bytes']} bytes physical memory.",
            "",
            "| Tool | Exact recorded version |",
            "|---|---|",
            *tool_rows,
            "",
            "| Dependency role | Lock path | SHA-256 |",
            "|---|---|---|",
            *lock_rows,
            "",
            "## Descriptive wall-time summaries",
            "",
            "Median and arithmetic mean are milliseconds. Relative standard deviation describes observed dispersion across the five raw repetitions; the raw asset retains minimum, maximum, sample standard deviation, exact commands, semantic digests, and every individual sample.",
            "",
            "| Case | Implementation | Samples | Median ms | Mean ms | Relative standard deviation |",
            "|---|---|---:|---:|---:|---:|",
            *timing_rows,
            "",
            "## Reproduce",
            "",
            "Check out the exact source revision above, install the recorded tool versions, and run:",
            "",
            "```console",
            "python3 scripts/run_benchmark_provenance.py --check",
            "python3 scripts/run_benchmark_provenance.py --build",
            "python3 scripts/run_benchmark_provenance.py --run --output benchmark-local-result.json",
            f"python3 scripts/publish_benchmark_results.py --build-assets --input benchmark-local-result.json --output-dir release-assets --release-tag {plan.release_tag}",
            "```",
            "",
        )
    )


def render_index(plan: PublicationPlan) -> str:
    """Render the committed exact-version release-asset index."""

    return "\n".join(
        (
            f"# Versioned Benchmark Results v{plan.release}",
            "",
            f"The dedicated `{plan.release_tag}` evidence release is the only publication path for `{plan.provenance_suite_id}`. Its tag target must be the exact clean source revision recorded inside the result.",
            "",
            "## Release assets",
            "",
            f"- [Complete machine-readable result]({plan.raw_url}) — semantic records, every preflight decision, all raw repetitions, statistical summaries, host/tool/build/lock/fixture provenance, and exact commands.",
            f"- [Concise human report]({plan.report_url}) — coverage, explicit rejections, recorded environment, and descriptive wall-time summaries derived from the raw result.",
            f"- [SHA-256 checksums]({plan.checksums_url}) — digest bindings for both result assets.",
            "",
            f"The `{plan.runner}` tag workflow rebuilds the pinned Python reference, release-mode candidate wheel, and locked release competitor adapter before it runs five round-robin repetitions. It publishes only when the exact semantic gate passes for a comparison; rejected comparisons remain in the raw and human assets without timings.",
            "",
            "These artifacts are one host observation, not a confidence interval, regression threshold, product ranking, or broad performance claim. Archived competitor revisions are historical comparison points and do not imply anything about current maintenance activity.",
            "",
            "## Reproduction boundary",
            "",
            "Use the source revision, tool versions, dependency-lock hashes, fixture hashes, build flags, and argument arrays inside the raw asset. The release tag is separate from package tags such as `v0.3.0`, so publishing benchmark evidence cannot trigger package registries.",
            "",
        )
    )


def _validate_timing_eligibility(
    plan: PublicationPlan,
    decisions: Sequence[Mapping[str, object]],
    timings: Sequence[Mapping[str, object]],
) -> None:
    scheduled_keys = {
        (scenario.id, f"{fixture_id}:{scenario.id}", implementation_id)
        for scenario in plan.provenance_plan.scenario_suite.scenarios
        for fixture_id in scenario.fixture_ids
        for implementation_id in scenario.timed_implementations
    }
    decision_by_key: dict[tuple[str, str, str], bool] = {}
    for decision in decisions:
        key = (
            _required_string(decision, "scenario_id", "preflight decision"),
            _required_string(decision, "case_id", "preflight decision"),
            _required_string(decision, "implementation_id", "preflight decision"),
        )
        if key in decision_by_key:
            raise BenchmarkResultError(f"duplicate preflight decision: {key}")
        eligible = decision.get("eligible_for_timing")
        if not isinstance(eligible, bool):
            raise BenchmarkResultError(f"preflight decision {key} lacks eligibility")
        reasons = decision.get("reasons")
        if not isinstance(reasons, list) or any(
            not isinstance(reason, str) or not reason for reason in reasons
        ):
            raise BenchmarkResultError(f"preflight decision {key} has invalid reasons")
        if eligible == bool(reasons):
            raise BenchmarkResultError(
                f"preflight decision {key} has inconsistent eligibility/reasons"
            )
        decision_by_key[key] = eligible

    timing_keys: set[tuple[str, str, str]] = set()
    for timing in timings:
        implementation = _required_mapping(timing, "implementation", "timing")
        key = (
            _required_string(timing, "scenario_id", "timing"),
            _required_string(timing, "case_id", "timing"),
            _required_string(implementation, "id", "timing implementation"),
        )
        if key not in scheduled_keys:
            raise BenchmarkResultError(f"unscheduled comparison was timed: {key}")
        timing_keys.add(key)

    for key, eligible in decision_by_key.items():
        if not eligible and key in timing_keys:
            raise BenchmarkResultError(f"rejected comparison was timed: {key}")
        if eligible and key in scheduled_keys and key not in timing_keys:
            raise BenchmarkResultError(f"eligible comparison lacks timings: {key}")
        if eligible and key not in scheduled_keys and key in timing_keys:
            raise BenchmarkResultError(f"semantic-only comparison was timed: {key}")
    for key in timing_keys:
        if key[2] == REFERENCE_IMPLEMENTATION:
            continue
        if decision_by_key.get(key) is not True:
            raise BenchmarkResultError(f"timing lacks an eligible preflight: {key}")


def _required_mapping(
    value: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise BenchmarkResultError(f"{context}.{key} must be an object")
    return item


def _required_string(value: Mapping[str, object], key: str, context: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or not item:
        raise BenchmarkResultError(f"{context}.{key} must be a non-empty string")
    return item


def _required_object_array(
    value: Mapping[str, object], key: str
) -> list[Mapping[str, object]]:
    items = value.get(key)
    if not isinstance(items, list) or any(not isinstance(item, dict) for item in items):
        raise BenchmarkResultError(f"{key} must be an array of objects")
    return items


def _canonical_json(value: object) -> str:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    except (TypeError, ValueError) as error:
        raise BenchmarkResultError("benchmark result is not finite JSON") from error


def _canonical_json_pretty(value: object) -> str:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
    except (TypeError, ValueError) as error:
        raise BenchmarkResultError("benchmark result is not finite JSON") from error


def _milliseconds(value: object) -> str:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value <= 0:
        raise BenchmarkResultError(f"wall-time summary is invalid: {value!r}")
    return f"{value / 1_000_000:.6f}"


def _markdown_text(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")
