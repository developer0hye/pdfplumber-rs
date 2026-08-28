"""Fail-closed lifecycle for published benchmark results (SCORE-009)."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_results

SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
AUDIT_URL_PATTERN = re.compile(
    r"^https://github\.com/developer0hye/pdfplumber-rs/actions/runs/[1-9][0-9]*$"
)
RETAINED = "retained"
WITHDRAWN = "withdrawn"
AUDIT_POLICY = "exact-semantic-reproduction"
WITHDRAWAL_POLICY = "remove-assets-tombstone-release-retain-tag"


class BenchmarkRetentionError(ValueError):
    """Raised when the result-retention contract itself is inconclusive."""


@dataclass(frozen=True)
class RetentionPlan:
    """Versioned identity, evidence, and state for one published result bundle."""

    id: str
    status: str
    release_tag: str
    source_revision: str
    raw_sha256: str
    report_sha256: str
    checksums_sha256: str
    audit_evidence_url: str
    audit_policy: str
    withdrawal_policy: str
    publication_plan: benchmark_results.PublicationPlan


@dataclass(frozen=True)
class RetentionDecision:
    """Machine-readable outcome of comparing a release with a completed rerun."""

    schema_version: int
    status: str
    result_id: str
    release_tag: str
    source_revision: str
    reasons: tuple[str, ...]
    semantic_record_count: int
    timed_group_count: int


def audit_repository(
    repo_root: Path,
    retention_path: Path,
    publication_path: Path,
    provenance_path: Path,
    scenario_path: Path,
    competitor_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> RetentionPlan:
    """Validate the retention manifest and all inherited publication inputs."""

    publication_plan = benchmark_results.audit_repository(
        repo_root,
        publication_path,
        provenance_path,
        scenario_path,
        competitor_path,
        corpus_path,
        policy_path,
        registry_path,
    )
    try:
        with retention_path.open("rb") as retention_file:
            source = tomllib.load(retention_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkRetentionError(
            f"cannot read retention manifest: {retention_path}"
        ) from error
    return validate_plan(source, publication_plan)


def validate_plan(
    source: Mapping[str, object],
    publication_plan: benchmark_results.PublicationPlan,
) -> RetentionPlan:
    """Validate one parsed retention manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkRetentionError("schema.version must be 1")
    result = _required_mapping(source, "result", "manifest")
    result_id = _required_string(result, "id", "result")
    expected_id = f"{publication_plan.id}-retention"
    if result_id != expected_id:
        raise BenchmarkRetentionError(f"result.id must be {expected_id!r}")
    if _required_string(result, "publication_id", "result") != publication_plan.id:
        raise BenchmarkRetentionError("retention names the wrong publication")
    release_tag = _required_string(result, "release_tag", "result")
    if release_tag != publication_plan.release_tag:
        raise BenchmarkRetentionError("retention and publication release tags differ")
    source_revision = _required_string(result, "source_revision", "result")
    if not SHA_PATTERN.fullmatch(source_revision):
        raise BenchmarkRetentionError("result.source_revision must be a full Git SHA")
    status = _required_string(result, "status", "result")
    if status not in {RETAINED, WITHDRAWN}:
        raise BenchmarkRetentionError("result.status must be retained or withdrawn")
    audit_policy = _required_string(result, "audit_policy", "result")
    if audit_policy != AUDIT_POLICY:
        raise BenchmarkRetentionError(
            f"result.audit_policy must be {AUDIT_POLICY!r}"
        )
    withdrawal_policy = _required_string(result, "withdrawal_policy", "result")
    if withdrawal_policy != WITHDRAWAL_POLICY:
        raise BenchmarkRetentionError(
            f"result.withdrawal_policy must be {WITHDRAWAL_POLICY!r}"
        )
    audit_evidence_url = _required_string(result, "audit_evidence_url", "result")
    if not AUDIT_URL_PATTERN.fullmatch(audit_evidence_url):
        raise BenchmarkRetentionError("result.audit_evidence_url is not canonical")

    digests = {
        name: _required_string(result, name, "result")
        for name in ("raw_sha256", "report_sha256", "checksums_sha256")
    }
    for name, digest in digests.items():
        if not SHA256_PATTERN.fullmatch(digest):
            raise BenchmarkRetentionError(f"result.{name} must be a SHA-256 digest")

    withdrawal = source.get("withdrawal")
    if status == RETAINED and withdrawal is not None:
        raise BenchmarkRetentionError("a retained result cannot have withdrawal metadata")
    if status == WITHDRAWN:
        if not isinstance(withdrawal, dict):
            raise BenchmarkRetentionError("a withdrawn result requires withdrawal metadata")
        reason = _required_string(withdrawal, "reason", "withdrawal")
        if not reason.strip():
            raise BenchmarkRetentionError("withdrawal.reason cannot be blank")
        evidence = _required_string(withdrawal, "audit_evidence_url", "withdrawal")
        if not AUDIT_URL_PATTERN.fullmatch(evidence):
            raise BenchmarkRetentionError(
                "withdrawal.audit_evidence_url is not canonical"
            )

    return RetentionPlan(
        id=result_id,
        status=status,
        release_tag=release_tag,
        source_revision=source_revision,
        raw_sha256=digests["raw_sha256"],
        report_sha256=digests["report_sha256"],
        checksums_sha256=digests["checksums_sha256"],
        audit_evidence_url=audit_evidence_url,
        audit_policy=audit_policy,
        withdrawal_policy=withdrawal_policy,
        publication_plan=publication_plan,
    )


def audit_release_assets(
    plan: RetentionPlan,
    published_directory: Path,
    reproduced_local_run: Mapping[str, object],
) -> RetentionDecision:
    """Decide whether a completed exact-tag rerun still validates the bundle."""

    publication = plan.publication_plan
    raw_path = published_directory / publication.raw_asset
    report_path = published_directory / publication.report_asset
    checksums_path = published_directory / publication.checksums_asset
    paths = (raw_path, report_path, checksums_path)
    if any(not path.is_file() for path in paths):
        missing = ", ".join(path.name for path in paths if not path.is_file())
        raise BenchmarkRetentionError(
            f"published asset download is incomplete; missing {missing}"
        )

    raw_bytes = raw_path.read_bytes()
    report_bytes = report_path.read_bytes()
    checksums_bytes = checksums_path.read_bytes()
    reasons: list[str] = []
    raw_sha256 = hashlib.sha256(raw_bytes).hexdigest()
    report_sha256 = hashlib.sha256(report_bytes).hexdigest()
    checksums_sha256 = hashlib.sha256(checksums_bytes).hexdigest()
    if raw_sha256 != plan.raw_sha256:
        reasons.append("published raw asset SHA-256 no longer matches the registry")
    if report_sha256 != plan.report_sha256:
        reasons.append("published report asset SHA-256 no longer matches the registry")
    if checksums_sha256 != plan.checksums_sha256:
        reasons.append("published checksum asset SHA-256 no longer matches the registry")
    expected_checksums = (
        f"{raw_sha256}  {publication.raw_asset}\n"
        f"{report_sha256}  {publication.report_asset}\n"
    ).encode()
    if checksums_bytes != expected_checksums:
        reasons.append("published checksum contents do not bind the downloaded assets")

    try:
        published = json.loads(raw_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError):
        published = None
        reasons.append("published raw asset is not valid UTF-8 JSON")
    if not isinstance(published, dict):
        if published is not None:
            reasons.append("published raw asset root is not an object")
        return _decision(plan, reasons, 0, 0)

    records = published.get("records")
    summaries = published.get("statistical_summaries")
    record_count = len(records) if isinstance(records, list) else 0
    timed_group_count = len(summaries) if isinstance(summaries, list) else 0

    try:
        regenerated = _regenerate_published_run(plan, published)
        if _canonical_json(regenerated) != _canonical_json(published):
            reasons.append("published raw asset is not the deterministic harness output")
        expected_report = benchmark_results.render_human_report(
            publication,
            published,
            raw_sha256,
        ).encode()
        if report_bytes != expected_report:
            reasons.append("published report is not the deterministic raw-result projection")
    except (benchmark_results.BenchmarkResultError, BenchmarkRetentionError) as error:
        reasons.append(f"published result contract failed: {error}")

    try:
        reproduced = benchmark_results.publish_run(
            publication,
            reproduced_local_run,
            release_tag=plan.release_tag,
            source_revision=plan.source_revision,
        )
    except benchmark_results.BenchmarkResultError as error:
        reasons.append(f"committed harness reproduction failed validation: {error}")
        return _decision(plan, reasons, record_count, timed_group_count)

    if _canonical_json(published.get("records")) != _canonical_json(
        reproduced.get("records")
    ):
        reasons.append("semantic records changed in the exact-tag reproduction")
    if _canonical_json(published.get("preflight_decisions")) != _canonical_json(
        reproduced.get("preflight_decisions")
    ):
        reasons.append(
            "output-equivalence decision changed in the exact-tag reproduction"
        )
    if _timing_identities(published.get("scenario_timings")) != _timing_identities(
        reproduced.get("scenario_timings")
    ):
        reasons.append("eligible timed key or semantic timing identity changed")
    if _summary_identities(
        published.get("statistical_summaries")
    ) != _summary_identities(reproduced.get("statistical_summaries")):
        reasons.append("timed summary group identity changed")

    return _decision(plan, reasons, record_count, timed_group_count)


def serialize_decision(decision: RetentionDecision) -> str:
    """Serialize one audit decision deterministically."""

    return (
        json.dumps(
            {
                "schema_version": decision.schema_version,
                "status": decision.status,
                "result_id": decision.result_id,
                "release_tag": decision.release_tag,
                "source_revision": decision.source_revision,
                "reasons": list(decision.reasons),
                "semantic_record_count": decision.semantic_record_count,
                "timed_group_count": decision.timed_group_count,
            },
            allow_nan=False,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def parse_decision(value: Mapping[str, object]) -> RetentionDecision:
    """Validate a serialized decision before a withdrawal mutation."""

    if value.get("schema_version") != 1:
        raise BenchmarkRetentionError("decision.schema_version must be 1")
    status = _required_string(value, "status", "decision")
    if status not in {"retain", "withdraw"}:
        raise BenchmarkRetentionError("decision.status must be retain or withdraw")
    reasons = value.get("reasons")
    if not isinstance(reasons, list) or any(
        not isinstance(reason, str) or not reason for reason in reasons
    ):
        raise BenchmarkRetentionError("decision.reasons must be strings")
    if (status == "retain") != (not reasons):
        raise BenchmarkRetentionError("decision status and reasons are inconsistent")
    semantic_record_count = value.get("semantic_record_count")
    timed_group_count = value.get("timed_group_count")
    for name, count in (
        ("semantic_record_count", semantic_record_count),
        ("timed_group_count", timed_group_count),
    ):
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise BenchmarkRetentionError(f"decision.{name} must be non-negative")
    return RetentionDecision(
        schema_version=1,
        status=status,
        result_id=_required_string(value, "result_id", "decision"),
        release_tag=_required_string(value, "release_tag", "decision"),
        source_revision=_required_string(value, "source_revision", "decision"),
        reasons=tuple(reasons),
        semantic_record_count=semantic_record_count,
        timed_group_count=timed_group_count,
    )


def withdrawal_asset_names(plan: RetentionPlan) -> tuple[str, str, str]:
    """Return the exact allowlist of result-bearing assets to remove."""

    publication = plan.publication_plan
    return publication.raw_asset, publication.report_asset, publication.checksums_asset


def withdrawal_decision_asset_name(plan: RetentionPlan) -> str:
    """Return the non-result audit record retained on a tombstoned Release."""

    return f"{plan.publication_plan.id}.withdrawal.json"


def render_withdrawal_tombstone(
    plan: RetentionPlan,
    decision: RetentionDecision,
    *,
    audit_evidence_url: str,
) -> str:
    """Render a Release tombstone that preserves why the result disappeared."""

    if decision.status != "withdraw" or not decision.reasons:
        raise BenchmarkRetentionError("a tombstone requires a withdrawal decision")
    if decision.result_id != plan.id or decision.release_tag != plan.release_tag:
        raise BenchmarkRetentionError("withdrawal decision names a different result")
    if decision.source_revision != plan.source_revision:
        raise BenchmarkRetentionError("withdrawal decision names a different source")
    if not AUDIT_URL_PATTERN.fullmatch(audit_evidence_url):
        raise BenchmarkRetentionError("withdrawal audit evidence URL is not canonical")
    reason_rows = tuple(f"- {reason}" for reason in decision.reasons)
    return "\n".join(
        (
            f"# Benchmark Results v{plan.publication_plan.release} — Withdrawn",
            "",
            "The three benchmark result assets were removed because a completed exact-tag audit no longer satisfied the committed reproduction and output-equivalence contract.",
            "",
            f"Exact source: `{plan.source_revision}`  ",
            f"Audit evidence: {audit_evidence_url}",
            "",
            "## Reasons",
            "",
            *reason_rows,
            "",
            "The immutable source tag is retained, together with this tombstone and the machine-readable withdrawal decision. This preserves the audit trail without continuing to publish invalid timing results.",
            "",
        )
    )


def render_index(plan: RetentionPlan) -> str:
    """Render the current public result or its withdrawal tombstone."""

    publication = plan.publication_plan
    if plan.status == WITHDRAWN:
        return "\n".join(
            (
                f"# Versioned Benchmark Results v{publication.release}",
                "",
                "Status: **withdrawn**",
                "",
                f"The timing assets for `{plan.release_tag}` are not published because they no longer satisfy the committed reproduction and exact output-equivalence contract. The immutable tag and Release tombstone preserve the audit trail.",
                "",
            )
        )
    base = benchmark_results.render_index(publication).rstrip()
    return "\n".join(
        (
            base,
            "",
            "## Validity and withdrawal",
            "",
            "Status: **retained**",
            "",
            f"The initial [confirmed reproduction and publication run]({plan.audit_evidence_url}) completed the committed exact-tag harness and exact output-equivalence gate. The registry binds the raw, report, and checksum assets to SHA-256 values `{plan.raw_sha256}`, `{plan.report_sha256}`, and `{plan.checksums_sha256}`.",
            "",
            "A scheduled read-only audit reruns the immutable tag and compares semantic records, preflight decisions, timed keys, fixture bindings, and semantic digests. Host identity and timing values may differ. A completed audit that changes any semantic result produces a machine-readable withdrawal decision; transient setup or network failures are inconclusive and cannot remove evidence.",
            "",
            "Withdrawal removes the three result-bearing assets and replaces the Release body with a tombstone under the verified `developer0hye` identity. It never deletes the source tag or the audit decision.",
            "",
            "## Regression alerts",
            "",
            "The separate [SCORE-013 regression policy](regressions-v0.3.0.md) uses this retained tag as its immutable baseline. It checks exact semantic and timing-eligibility identities before applying its paired-run noise rule; alert decisions do not alter these Release assets.",
            "",
        )
    )


def _regenerate_published_run(
    plan: RetentionPlan,
    published: Mapping[str, object],
) -> dict[str, object]:
    if published.get("publication_status") != "release-artifact":
        raise BenchmarkRetentionError("published status is not release-artifact")
    local_keys = benchmark_results.LOCAL_RUN_KEYS
    if not local_keys.issubset(published):
        raise BenchmarkRetentionError("published raw asset lacks local-run fields")
    local = {key: published[key] for key in local_keys}
    local["publication_status"] = "local-unpublished"
    return benchmark_results.publish_run(
        plan.publication_plan,
        local,
        release_tag=plan.release_tag,
        source_revision=plan.source_revision,
    )


def _decision(
    plan: RetentionPlan,
    reasons: Sequence[str],
    semantic_record_count: int,
    timed_group_count: int,
) -> RetentionDecision:
    unique_reasons = tuple(dict.fromkeys(reasons))
    return RetentionDecision(
        schema_version=1,
        status="withdraw" if unique_reasons else "retain",
        result_id=plan.id,
        release_tag=plan.release_tag,
        source_revision=plan.source_revision,
        reasons=unique_reasons,
        semantic_record_count=semantic_record_count,
        timed_group_count=timed_group_count,
    )


def _timing_identities(value: object) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        return ("<invalid-timings>",)
    identities: set[str] = set()
    for timing in value:
        identity = {
            key: item
            for key, item in timing.items()
            if key not in {"repetition", "wall_time_ns"}
        }
        identities.add(_canonical_json(identity))
    return tuple(sorted(identities))


def _summary_identities(value: object) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        return ("<invalid-summaries>",)
    measured_fields = {
        "minimum_wall_time_ns",
        "median_wall_time_ns",
        "arithmetic_mean_wall_time_ns",
        "maximum_wall_time_ns",
        "sample_standard_deviation_wall_time_ns",
        "relative_standard_deviation",
        "samples_sha256",
    }
    identities = [
        _canonical_json(
            {key: item for key, item in summary.items() if key not in measured_fields}
        )
        for summary in value
    ]
    return tuple(sorted(identities))


def _required_mapping(
    value: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise BenchmarkRetentionError(f"{context}.{key} must be an object")
    return item


def _required_string(value: Mapping[str, object], key: str, context: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or not item:
        raise BenchmarkRetentionError(f"{context}.{key} must be a non-empty string")
    return item


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
        raise BenchmarkRetentionError("benchmark evidence is not finite JSON") from error
