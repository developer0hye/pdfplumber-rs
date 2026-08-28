"""Build a compact public scorecard from complete machine parity reports."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from dataclasses import dataclass
from typing import Mapping, Sequence

from compat.harness import corpus_index, machine_report


SCHEMA_VERSION: int = 1
GENERATOR: str = "scripts/generate_compatibility_scorecard.py"
PAGE_APIS: tuple[str, ...] = machine_report.PAGE_APIS
STATUSES: tuple[str, ...] = (
    "exact",
    "approved_delta",
    "unsupported",
    "reference_failure",
    "candidate_failure",
    "not_tested",
)
STATUS_VOCABULARY: dict[str, str] = {
    "exact": "Reference and candidate results are structurally equal.",
    "approved_delta": "The exact observed difference has a reviewed approval entry.",
    "unsupported": "The candidate explicitly reports that the API is unsupported.",
    "reference_failure": "The pinned reference could not produce a comparable result.",
    "candidate_failure": "The candidate failed or differed without an approved delta.",
    "not_tested": "No compatibility comparison was executed for this scope.",
}
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class ScorecardError(ValueError):
    """A parity result cannot be represented without losing its identity."""


@dataclass(frozen=True)
class Platform:
    """One exact execution platform, independent of the tested artifact."""

    id: str
    system: str
    release: str
    machine: str
    python_version: str


@dataclass(frozen=True)
class RunInput:
    """One observed parity run or one explicitly untested coverage cell."""

    id: str
    platform: Platform
    artifact_type: str
    artifact_name: str | None = None
    artifact_sha256: str | None = None
    command: str | None = None
    report: Mapping[str, object] | None = None
    not_tested_reason: str | None = None
    evidence: tuple[str, ...] = ()


def build(
    *,
    subject_version: str,
    subject_revision: str,
    corpus: corpus_index.CorpusIndex,
    corpus_sha256: str,
    runs: Sequence[RunInput],
) -> dict[str, object]:
    """Build a deterministic scorecard without collapsing domain outcomes."""

    _require_non_empty(subject_version, "subject version")
    if HEX_40.fullmatch(subject_revision) is None:
        raise ScorecardError("subject revision must be a full lowercase Git SHA")
    if SHA256.fullmatch(corpus_sha256) is None:
        raise ScorecardError("corpus SHA-256 must be a lowercase digest")
    if not runs:
        raise ScorecardError("at least one platform/artifact run is required")

    fixture_by_path = {fixture.path: fixture for fixture in corpus.fixtures}
    class_by_id = {collection.id: collection for collection in corpus.collections}
    run_ids: set[str] = set()
    platform_by_id: dict[str, Platform] = {}
    target: dict[str, object] | None = None
    run_records: list[dict[str, object]] = []
    observations: list[dict[str, object]] = []
    option_dimensions: dict[str, dict[str, object]] = {}
    page_dimensions: dict[tuple[str, int], dict[str, object]] = {}
    api_ids: set[str] = set(PAGE_APIS)

    for run in runs:
        _validate_run(run)
        if run.id in run_ids:
            raise ScorecardError(f"duplicate run ID: {run.id}")
        run_ids.add(run.id)
        existing_platform = platform_by_id.get(run.platform.id)
        if existing_platform is not None and existing_platform != run.platform:
            raise ScorecardError(
                f"platform ID {run.platform.id} has conflicting metadata"
            )
        platform_by_id[run.platform.id] = run.platform

        if run.report is None:
            run_records.append(_not_tested_run_record(run))
            observations.append(
                {
                    "id": f"{run.id}::run",
                    "run_id": run.id,
                    "kind": "run",
                    "platform_id": run.platform.id,
                    "artifact_type": run.artifact_type,
                    "status": "not_tested",
                    "reason": run.not_tested_reason,
                    "evidence": list(run.evidence),
                }
            )
            continue

        report_target = _required_mapping(run.report, "target", f"run {run.id}")
        copied_target = copy.deepcopy(dict(report_target))
        if target is None:
            target = copied_target
        elif copied_target != target:
            raise ScorecardError(f"run {run.id} targets a different upstream")

        run_records.append(_observed_run_record(run))
        run_observations, run_options, run_pages, run_apis = _report_observations(
            run,
            fixture_by_path,
        )
        observations.extend(run_observations)
        api_ids.update(run_apis)
        for option_id, option in run_options.items():
            existing_option = option_dimensions.get(option_id)
            if existing_option is not None and existing_option != option:
                raise ScorecardError(
                    f"option ID {option_id} has conflicting identity fields"
                )
            option_dimensions[option_id] = option
        page_dimensions.update(run_pages)

    if target is None:
        raise ScorecardError("at least one observed parity report is required")

    observations.sort(key=_observation_sort_key)
    dimensions: dict[str, object] = {
        "apis": sorted(api_ids),
        "options": [option_dimensions[key] for key in sorted(option_dimensions)],
        "fixture_classes": [
            {
                "id": collection.id,
                "description": collection.description,
            }
            for collection in sorted(corpus.collections, key=lambda value: value.id)
        ],
        "pages": [page_dimensions[key] for key in sorted(page_dimensions)],
        "platforms": [
            _platform_record(platform_by_id[key]) for key in sorted(platform_by_id)
        ],
        "artifact_types": sorted({run.artifact_type for run in runs}),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_by": GENERATOR,
        "subject": {
            "project": "pdfplumber-rs",
            "version": subject_version,
            "revision": subject_revision,
        },
        "target": target,
        "corpus": {
            "index": "compat/fixture-provenance.toml",
            "sha256": corpus_sha256,
            "fixture_count": len(corpus.fixtures),
        },
        "status_vocabulary": copy.deepcopy(STATUS_VOCABULARY),
        "runs": run_records,
        "dimensions": dimensions,
        "observations": observations,
        "summary": _summary(observations),
    }


def render(scorecard: Mapping[str, object]) -> str:
    """Return stable JSON suitable for versioned release artifacts."""

    return json.dumps(
        scorecard,
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
    ) + "\n"


def _validate_run(run: RunInput) -> None:
    _require_non_empty(run.id, "run ID")
    _require_non_empty(run.platform.id, f"run {run.id} platform ID")
    for field, value in (
        ("system", run.platform.system),
        ("release", run.platform.release),
        ("machine", run.platform.machine),
        ("python_version", run.platform.python_version),
        ("artifact type", run.artifact_type),
    ):
        _require_non_empty(value, f"run {run.id} {field}")
    if run.report is None:
        _require_non_empty(run.not_tested_reason, f"run {run.id} not-tested reason")
        if run.artifact_name is not None or run.artifact_sha256 is not None:
            raise ScorecardError(
                f"not-tested run {run.id} cannot claim a parity artifact"
            )
        return
    _require_non_empty(run.artifact_name, f"run {run.id} artifact name")
    if not isinstance(run.artifact_sha256, str) or SHA256.fullmatch(
        run.artifact_sha256
    ) is None:
        raise ScorecardError(f"run {run.id} artifact SHA-256 is invalid")
    _require_non_empty(run.command, f"run {run.id} command")
    if run.not_tested_reason is not None:
        raise ScorecardError(f"observed run {run.id} has a not-tested reason")
    if run.report.get("schema_version") != machine_report.SCHEMA_VERSION:
        raise ScorecardError(f"run {run.id} has an unsupported parity schema")


def _observed_run_record(run: RunInput) -> dict[str, object]:
    assert run.report is not None
    assert run.artifact_name is not None
    assert run.artifact_sha256 is not None
    assert run.command is not None
    record: dict[str, object] = {
        "id": run.id,
        "status": "observed",
        "platform_id": run.platform.id,
        "artifact_type": run.artifact_type,
        "artifact_name": run.artifact_name,
        "artifact_sha256": run.artifact_sha256,
        "command": run.command,
        "parity_report_sha256": _digest(run.report),
    }
    report_status = run.report.get("status")
    if isinstance(report_status, str) and report_status:
        record["parity_report_status"] = report_status
    if run.evidence:
        record["evidence"] = list(run.evidence)
    return record


def _not_tested_run_record(run: RunInput) -> dict[str, object]:
    record: dict[str, object] = {
        "id": run.id,
        "status": "not_tested",
        "platform_id": run.platform.id,
        "artifact_type": run.artifact_type,
        "reason": run.not_tested_reason,
    }
    if run.evidence:
        record["evidence"] = list(run.evidence)
    return record


def _report_observations(
    run: RunInput,
    fixture_by_path: Mapping[str, corpus_index.Fixture],
) -> tuple[
    list[dict[str, object]],
    dict[str, dict[str, object]],
    dict[tuple[str, int], dict[str, object]],
    set[str],
]:
    assert run.report is not None
    observations: list[dict[str, object]] = []
    options: dict[str, dict[str, object]] = {}
    pages: dict[tuple[str, int], dict[str, object]] = {}
    apis: set[str] = set()

    raw_fixtures = run.report.get("fixtures")
    if not isinstance(raw_fixtures, list):
        raise ScorecardError(f"run {run.id} fixtures must be a list")
    for raw_fixture in sorted(raw_fixtures, key=_fixture_sort_key):
        if not isinstance(raw_fixture, dict):
            raise ScorecardError(f"run {run.id} contains a non-object fixture")
        fixture_id = _required_string(raw_fixture, "fixture_id", "fixture")
        fixture = fixture_by_path.get(fixture_id)
        if fixture is None:
            raise ScorecardError(f"unknown fixture in run {run.id}: {fixture_id}")
        fixture_status = _required_string(raw_fixture, "status", fixture_id)
        if fixture_status != "compared":
            observations.append(
                _fixture_failure_observation(run, fixture, raw_fixture, fixture_status)
            )
            continue
        raw_pages = raw_fixture.get("pages")
        if not isinstance(raw_pages, list):
            raise ScorecardError(f"{fixture_id} pages must be a list")
        for raw_page in sorted(raw_pages, key=_page_sort_key):
            if not isinstance(raw_page, dict):
                raise ScorecardError(f"{fixture_id} contains a non-object page")
            page_number = _page_number(raw_page, fixture_id)
            pages[(fixture_id, page_number)] = _page_dimension(fixture, page_number)
            page_status = _required_string(
                raw_page,
                "status",
                f"{fixture_id} page {page_number}",
            )
            raw_apis = raw_page.get("apis")
            if not isinstance(raw_apis, dict):
                raise ScorecardError(
                    f"{fixture_id} page {page_number} APIs must be an object"
                )
            missing = sorted(set(PAGE_APIS) - set(raw_apis))
            extra = sorted(set(raw_apis) - set(PAGE_APIS))
            if missing or extra:
                raise ScorecardError(
                    f"{fixture_id} page {page_number} API identities differ; "
                    f"missing={missing}, extra={extra}"
                )
            for api in PAGE_APIS:
                raw_api = raw_apis[api]
                if not isinstance(raw_api, dict):
                    raise ScorecardError(f"{fixture_id} page {page_number} {api} is invalid")
                observations.append(
                    _api_observation(
                        run,
                        fixture,
                        page_number,
                        page_status,
                        api,
                        raw_api,
                    )
                )
                apis.add(api)

    raw_options = run.report.get("options")
    if not isinstance(raw_options, list):
        raise ScorecardError(f"run {run.id} options must be a list")
    for raw_option in sorted(raw_options, key=_option_sort_key):
        if not isinstance(raw_option, dict):
            raise ScorecardError(f"run {run.id} contains a non-object option")
        option_id = _required_string(raw_option, "id", "option")
        api = _required_string(raw_option, "api", f"option {option_id}")
        fixture_id = _required_string(
            raw_option,
            "fixture_path",
            f"option {option_id}",
        )
        fixture = fixture_by_path.get(fixture_id)
        if fixture is None:
            raise ScorecardError(
                f"unknown fixture in run {run.id} option {option_id}: {fixture_id}"
            )
        page_number = _page_number(raw_option, f"option {option_id}")
        pages[(fixture_id, page_number)] = _page_dimension(fixture, page_number)
        option_identity = {
            "id": option_id,
            "api": api,
            "fixture_id": fixture_id,
            "fixture_class": fixture.collection,
            "page_number": page_number,
            "covers": copy.deepcopy(raw_option.get("covers")),
            "options": copy.deepcopy(raw_option.get("options")),
        }
        options[option_id] = option_identity
        observations.append(
            _option_observation(
                run,
                fixture,
                page_number,
                option_id,
                api,
                raw_option,
            )
        )
        apis.add(api)
    return observations, options, pages, apis


def _fixture_failure_observation(
    run: RunInput,
    fixture: corpus_index.Fixture,
    raw_fixture: Mapping[str, object],
    fixture_status: str,
) -> dict[str, object]:
    status = {
        "python_failed": "reference_failure",
        "reference_failed": "reference_failure",
        "rust_failed": "candidate_failure",
        "candidate_failed": "candidate_failure",
    }.get(fixture_status)
    if status is None:
        raise ScorecardError(
            f"fixture {fixture.path} has unknown status {fixture_status!r}"
        )
    record = _observation_base(run, fixture, "fixture", status)
    record["id"] = f"{run.id}::fixture::{fixture.path}"
    record["evidence_sha256"] = _digest(raw_fixture)
    if "error" in raw_fixture:
        record["error"] = copy.deepcopy(raw_fixture["error"])
    return record


def _api_observation(
    run: RunInput,
    fixture: corpus_index.Fixture,
    page_number: int,
    page_status: str,
    api: str,
    raw_api: Mapping[str, object],
) -> dict[str, object]:
    raw_status = _required_string(raw_api, "status", f"API {api}")
    if page_status == "missing_in_python":
        status = "reference_failure"
    elif page_status == "missing_in_rust":
        status = "candidate_failure"
    elif page_status != "compared":
        status = "not_tested"
    elif raw_status in {"equal", "exact"}:
        status = "exact"
    elif raw_status == "unsupported":
        status = "unsupported"
    elif raw_status == "not_compared":
        status = "not_tested"
    elif raw_status == "different":
        delta_gate = raw_api.get("delta_gate")
        status = (
            "approved_delta"
            if isinstance(delta_gate, dict) and delta_gate.get("status") == "approved"
            else "candidate_failure"
        )
    else:
        raise ScorecardError(f"API {api} has unknown status {raw_status!r}")

    record = _observation_base(run, fixture, "api", status)
    record.update(
        {
            "id": f"{run.id}::api::{fixture.path}::{page_number}::{api}",
            "page_number": page_number,
            "api_id": api,
            "evidence_sha256": _digest(raw_api),
        }
    )
    comparison = raw_api.get("comparison")
    if isinstance(comparison, dict):
        for key in ("task_id", "reason"):
            if key in comparison:
                record[key] = copy.deepcopy(comparison[key])
    delta_gate = raw_api.get("delta_gate")
    if isinstance(delta_gate, dict):
        record["delta_gate"] = copy.deepcopy(delta_gate)
    return record


def _option_observation(
    run: RunInput,
    fixture: corpus_index.Fixture,
    page_number: int,
    option_id: str,
    api: str,
    raw_option: Mapping[str, object],
) -> dict[str, object]:
    reference = raw_option.get("reference")
    candidate = raw_option.get("candidate")
    comparison = _required_mapping(
        raw_option,
        "comparison",
        f"option {option_id}",
    )
    comparison_status = _required_string(
        comparison,
        "status",
        f"option {option_id} comparison",
    )
    if isinstance(reference, dict) and reference.get("status") not in {None, "ok"}:
        status = "reference_failure"
    elif isinstance(candidate, dict) and candidate.get("status") == "blocked":
        status = "not_tested"
    elif isinstance(candidate, dict) and candidate.get("status") not in {None, "ok"}:
        status = "candidate_failure"
    elif comparison_status == "equal":
        status = "exact"
    elif comparison_status == "different":
        status = "candidate_failure"
    elif comparison_status == "not_compared":
        status = "not_tested"
    else:
        raise ScorecardError(
            f"option {option_id} has unknown comparison status {comparison_status!r}"
        )
    record = _observation_base(run, fixture, "option", status)
    record.update(
        {
            "id": f"{run.id}::option::{option_id}",
            "page_number": page_number,
            "api_id": api,
            "option_id": option_id,
            "evidence_sha256": _digest(raw_option),
        }
    )
    return record


def _observation_base(
    run: RunInput,
    fixture: corpus_index.Fixture,
    kind: str,
    status: str,
) -> dict[str, object]:
    return {
        "run_id": run.id,
        "kind": kind,
        "platform_id": run.platform.id,
        "artifact_type": run.artifact_type,
        "fixture_class": fixture.collection,
        "fixture_id": fixture.path,
        "status": status,
    }


def _summary(observations: Sequence[Mapping[str, object]]) -> dict[str, object]:
    return {
        "status_counts": _status_counts(observations),
        "by_api": _group_summary(observations, "api_id"),
        "by_option": _group_summary(observations, "option_id"),
        "by_fixture_class": _group_summary(observations, "fixture_class"),
        "by_page": _page_summary(observations),
        "by_platform": _group_summary(observations, "platform_id"),
        "by_artifact_type": _group_summary(observations, "artifact_type"),
    }


def _group_summary(
    observations: Sequence[Mapping[str, object]],
    field: str,
) -> list[dict[str, object]]:
    groups: dict[str, list[Mapping[str, object]]] = {}
    for observation in observations:
        value = observation.get(field)
        if isinstance(value, str) and value:
            groups.setdefault(value, []).append(observation)
    return [
        {"id": key, "status_counts": _status_counts(groups[key])}
        for key in sorted(groups)
    ]


def _page_summary(
    observations: Sequence[Mapping[str, object]],
) -> list[dict[str, object]]:
    groups: dict[tuple[str, int], list[Mapping[str, object]]] = {}
    for observation in observations:
        fixture_id = observation.get("fixture_id")
        page_number = observation.get("page_number")
        if (
            isinstance(fixture_id, str)
            and isinstance(page_number, int)
            and not isinstance(page_number, bool)
        ):
            groups.setdefault((fixture_id, page_number), []).append(observation)
    return [
        {
            "id": f"{fixture_id}#page={page_number}",
            "fixture_id": fixture_id,
            "page_number": page_number,
            "status_counts": _status_counts(groups[(fixture_id, page_number)]),
        }
        for fixture_id, page_number in sorted(groups)
    ]


def _status_counts(
    observations: Sequence[Mapping[str, object]],
) -> dict[str, int]:
    counts = {status: 0 for status in STATUSES}
    for observation in observations:
        status = observation.get("status")
        if not isinstance(status, str) or status not in counts:
            raise ScorecardError(f"unknown observation status: {status!r}")
        counts[status] += 1
    return counts


def _page_dimension(
    fixture: corpus_index.Fixture,
    page_number: int,
) -> dict[str, object]:
    return {
        "fixture_class": fixture.collection,
        "fixture_id": fixture.path,
        "page_number": page_number,
    }


def _platform_record(platform: Platform) -> dict[str, str]:
    return {
        "id": platform.id,
        "system": platform.system,
        "release": platform.release,
        "machine": platform.machine,
        "python_version": platform.python_version,
    }


def _required_mapping(
    data: Mapping[str, object],
    key: str,
    context: str,
) -> Mapping[str, object]:
    value = data.get(key)
    if not isinstance(value, dict):
        raise ScorecardError(f"{context} {key} must be an object")
    return value


def _required_string(data: Mapping[str, object], key: str, context: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise ScorecardError(f"{context} has no non-empty {key}")
    return value


def _require_non_empty(value: object, label: str) -> None:
    if not isinstance(value, str) or not value:
        raise ScorecardError(f"{label} must be a non-empty string")


def _page_number(data: Mapping[str, object], context: str) -> int:
    value = data.get("page_number")
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ScorecardError(f"{context} has an invalid page number")
    return value


def _digest(value: object) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _fixture_sort_key(value: object) -> str:
    if not isinstance(value, dict):
        raise ScorecardError("fixture record must be an object")
    return _required_string(value, "fixture_id", "fixture")


def _page_sort_key(value: object) -> int:
    if not isinstance(value, dict):
        raise ScorecardError("page record must be an object")
    return _page_number(value, "page")


def _option_sort_key(value: object) -> str:
    if not isinstance(value, dict):
        raise ScorecardError("option record must be an object")
    return _required_string(value, "id", "option")


def _observation_sort_key(value: Mapping[str, object]) -> str:
    identifier = value.get("id")
    if not isinstance(identifier, str):
        raise ScorecardError("observation has no string ID")
    return identifier
