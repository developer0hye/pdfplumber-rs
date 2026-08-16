"""Versioned, deterministic machine-readable parity reports (PARITY-017)."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from typing import Any, Mapping

from compat.harness import approved_deltas, upstream


SCHEMA_VERSION: int = 1
GENERATOR: str = "scripts/parity_report.py --json <path>"
PAGE_APIS: tuple[str, ...] = (
    "chars",
    "words",
    "page_text",
    "layout_text",
    "simple_text",
    "text_lines",
    "search",
    "tables",
    "annotations",
    "hyperlinks",
    "structure_tree",
)
OPTION_IDENTITY_FIELDS: tuple[str, ...] = (
    "id",
    "domain",
    "api",
    "fixture_path",
    "fixture_sha256",
    "page_number",
    "covers",
    "options",
    "arguments",
)


class MachineReportError(RuntimeError):
    """An input cannot be represented without omitting parity evidence."""


def build(
    fixture_results: Mapping[str, object],
    reference_options: Mapping[str, object],
    candidate_options: Mapping[str, object] | None = None,
    delta_gate: approved_deltas.GateResult | None = None,
) -> dict[str, object]:
    """Build one report with explicit fixture/page/API and option outcomes."""
    target = upstream.load_target()
    _validate_option_target(reference_options, target)
    if candidate_options is not None:
        _validate_option_target(candidate_options, target)

    delta_dispositions = _delta_dispositions(delta_gate)
    fixtures, fixture_summary = _fixture_records(
        fixture_results,
        delta_dispositions,
    )
    options, option_summary = _option_records(reference_options, candidate_options)
    summary = {**fixture_summary, **option_summary}
    failed = (
        summary["fixtures_failed"] > 0
        or summary["pages_not_compared"] > 0
        or summary["api_results_unsupported"] > 0
        or summary["option_cases_compared"] != summary["option_cases_total"]
        or summary["option_cases_different"] > 0
        or (delta_gate is not None and delta_gate.exit_code != 0)
    )

    reference_environment = {
        key: copy.deepcopy(reference_options[key])
        for key in ("python_version", "lockfile_sha256", "generated_by")
        if key in reference_options
    }
    if candidate_options is None:
        candidate_environment: dict[str, object] = {
            "status": "blocked",
            "task_id": "PYAPI-002",
        }
    else:
        candidate_environment = {
            "status": "provided",
            **{
                key: copy.deepcopy(candidate_options[key])
                for key in ("python_version", "lockfile_sha256", "generated_by")
                if key in candidate_options
            },
        }
    report: dict[str, object] = {
        "schema_version": SCHEMA_VERSION,
        "generated_by": GENERATOR,
        "target": {
            "project": target.project,
            "version": target.version,
            "tag": target.tag,
            "commit": target.commit,
            "repository": target.repository,
        },
        "reference_environment": reference_environment,
        "candidate_environment": candidate_environment,
        "approved_delta_gate": _delta_gate_record(delta_gate),
        "status": "failed" if failed else "passed",
        "summary": summary,
        "fixtures": fixtures,
        "options": options,
    }
    return report


def render(report: Mapping[str, object]) -> str:
    """Return stable JSON bytes suitable for CI artifacts and diffs."""
    return json.dumps(
        report,
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
    ) + "\n"


def _fixture_records(
    fixture_results: Mapping[str, object],
    delta_dispositions: Mapping[tuple[str, int, str], Mapping[str, object]],
) -> tuple[list[dict[str, object]], dict[str, int]]:
    records: list[dict[str, object]] = []
    pages_compared = 0
    pages_not_compared = 0
    api_counts = {
        "equal": 0,
        "different": 0,
        "unsupported": 0,
        "not_compared": 0,
    }
    fixtures_compared = 0
    fixtures_failed = 0

    for fixture_id in sorted(fixture_results):
        raw = fixture_results[fixture_id]
        if not isinstance(raw, dict):
            raise MachineReportError(f"fixture result must be a table: {fixture_id}")
        status = _required_string(raw, "status", fixture_id)
        record: dict[str, object] = {
            "fixture_id": fixture_id,
            "status": status,
        }
        if status != "compared":
            fixtures_failed += 1
            for key, value in raw.items():
                if key != "status":
                    record[key] = copy.deepcopy(value)
            records.append(record)
            continue

        fixtures_compared += 1
        for key in (
            "page_count_expected",
            "page_count_actual",
            "page_count_equal",
        ):
            if key not in raw:
                raise MachineReportError(f"{fixture_id} is missing {key}")
            record[key] = copy.deepcopy(raw[key])
        raw_pages = raw.get("pages")
        if not isinstance(raw_pages, list):
            raise MachineReportError(f"{fixture_id}.pages must be a list")
        pages: list[dict[str, object]] = []
        for raw_page in sorted(raw_pages, key=_page_sort_key):
            if not isinstance(raw_page, dict):
                raise MachineReportError(f"{fixture_id} contains a non-table page")
            page_number = raw_page.get("page_number")
            if isinstance(page_number, bool) or not isinstance(page_number, int):
                raise MachineReportError(f"{fixture_id} has an invalid page number")
            page_status = _required_string(
                raw_page,
                "status",
                f"{fixture_id} page {page_number}",
            )
            apis: dict[str, object] = {}
            if page_status == "compared":
                pages_compared += 1
                for api in PAGE_APIS:
                    if api not in raw_page:
                        raise MachineReportError(
                            f"{fixture_id} page {page_number} is missing API {api}"
                        )
                    comparison = copy.deepcopy(raw_page[api])
                    outcome = _api_status(api, comparison)
                    api_counts[outcome] += 1
                    apis[api] = {
                        "status": outcome,
                        "comparison": comparison,
                    }
                    disposition = delta_dispositions.get(
                        (fixture_id, page_number, api)
                    )
                    if disposition is not None:
                        apis[api]["delta_gate"] = copy.deepcopy(disposition)
            else:
                pages_not_compared += 1
                for api in PAGE_APIS:
                    api_counts["not_compared"] += 1
                    apis[api] = {
                        "status": "not_compared",
                        "reason": page_status,
                    }
            pages.append(
                {
                    "page_number": page_number,
                    "status": page_status,
                    "apis": apis,
                }
            )
        record["pages"] = pages
        records.append(record)

    return records, {
        "fixtures_total": len(records),
        "fixtures_compared": fixtures_compared,
        "fixtures_failed": fixtures_failed,
        "pages_compared": pages_compared,
        "pages_not_compared": pages_not_compared,
        "api_results_total": sum(api_counts.values()),
        "api_results_equal": api_counts["equal"],
        "api_results_different": api_counts["different"],
        "api_results_unsupported": api_counts["unsupported"],
        "api_results_not_compared": api_counts["not_compared"],
    }


def _delta_dispositions(
    gate: approved_deltas.GateResult | None,
) -> dict[tuple[str, int, str], dict[str, object]]:
    if gate is None:
        return {}
    dispositions: dict[tuple[str, int, str], dict[str, object]] = {}
    for observed, approval in gate.approved:
        key = (observed.fixture, observed.page, observed.api)
        dispositions[key] = {
            "status": "approved",
            "id": approval.identifier,
            "upstream_sha256": observed.upstream_sha256,
            "rust_sha256": observed.rust_sha256,
        }
    for observed in gate.unregistered:
        key = (observed.fixture, observed.page, observed.api)
        if key in dispositions:
            raise MachineReportError(
                f"duplicate delta disposition for {key[0]} page {key[1]} {key[2]}"
            )
        dispositions[key] = {
            "status": "unregistered",
            "upstream_sha256": observed.upstream_sha256,
            "rust_sha256": observed.rust_sha256,
        }
    return dispositions


def _delta_gate_record(
    gate: approved_deltas.GateResult | None,
) -> dict[str, object]:
    if gate is None:
        return {"status": "not_provided"}
    return {
        "status": "failed" if gate.exit_code else "passed",
        "approved": len(gate.approved),
        "unregistered": len(gate.unregistered),
        "stale": len(gate.stale),
        "stale_entries": [
            {
                "id": delta.identifier,
                "fixture": delta.fixture,
                "page": delta.page,
                "api": delta.api,
                "upstream_sha256": delta.upstream_sha256,
                "rust_sha256": delta.rust_sha256,
            }
            for delta in gate.stale
        ],
    }


def _api_status(api: str, comparison: object) -> str:
    if not isinstance(comparison, dict):
        raise MachineReportError(f"{api} comparison must be a table")
    comparison_status = comparison.get("status", "compared")
    if comparison_status in {"unsupported", "unsupported_in_rust"}:
        return "unsupported"
    if comparison_status != "compared":
        raise MachineReportError(
            f"{api} comparison has unknown status {comparison_status!r}"
        )
    if api == "chars":
        dictionary = comparison.get("dictionary")
        if not isinstance(dictionary, dict):
            raise MachineReportError("chars comparison is missing dictionary result")
        equal = (
            comparison.get("count_expected") == comparison.get("count_actual")
            and comparison.get("text_order_equal") is True
            and comparison.get("box_order_equal") is True
            and dictionary.get("structure_equal") is True
        )
        return "equal" if equal else "different"
    if comparison.get("equal") is True:
        return "equal"
    if "equal" not in comparison:
        raise MachineReportError(f"{api} comparison is missing exact equality")
    return "different"


def _option_records(
    reference: Mapping[str, object],
    candidate: Mapping[str, object] | None,
) -> tuple[list[dict[str, object]], dict[str, int]]:
    reference_cases = _case_index(reference, "reference")
    candidate_cases = None if candidate is None else _case_index(candidate, "candidate")
    if candidate_cases is not None and set(candidate_cases) != set(reference_cases):
        missing = sorted(set(reference_cases) - set(candidate_cases))
        extra = sorted(set(candidate_cases) - set(reference_cases))
        raise MachineReportError(
            f"candidate option IDs differ; missing={missing}, extra={extra}"
        )

    records: list[dict[str, object]] = []
    compared = 0
    equal = 0
    different = 0
    blocked = 0
    for identifier in sorted(reference_cases):
        reference_case = reference_cases[identifier]
        base = {
            field: copy.deepcopy(reference_case[field])
            for field in OPTION_IDENTITY_FIELDS
        }
        reference_outcome = _case_outcome(reference_case, reference, "reference")
        base["reference"] = reference_outcome
        if candidate_cases is None:
            blocked += 1
            base["candidate"] = {"status": "blocked", "task_id": "PYAPI-002"}
            base["comparison"] = {
                "status": "not_compared",
                "task_id": "PYAPI-002",
            }
        else:
            candidate_case = candidate_cases[identifier]
            for field in OPTION_IDENTITY_FIELDS:
                if candidate_case.get(field) != reference_case.get(field):
                    raise MachineReportError(
                        f"candidate option {identifier} changed identity field {field}"
                    )
            candidate_outcome = _case_outcome(
                candidate_case,
                candidate,
                "candidate",
            )
            is_equal = candidate_outcome == reference_outcome
            compared += 1
            equal += int(is_equal)
            different += int(not is_equal)
            base["candidate"] = candidate_outcome
            base["comparison"] = {
                "status": "equal" if is_equal else "different",
                "equal": is_equal,
            }
        records.append(base)

    return records, {
        "option_cases_total": len(records),
        "option_cases_compared": compared,
        "option_cases_equal": equal,
        "option_cases_different": different,
        "option_cases_blocked": blocked,
    }


def _case_index(
    snapshot: Mapping[str, object],
    label: str,
) -> dict[str, dict[str, object]]:
    raw_cases = snapshot.get("cases")
    if not isinstance(raw_cases, list):
        raise MachineReportError(f"{label} option cases must be a list")
    index: dict[str, dict[str, object]] = {}
    for position, case in enumerate(raw_cases):
        if not isinstance(case, dict):
            raise MachineReportError(f"{label} option case {position} must be a table")
        identifier = _required_string(case, "id", f"{label} option case {position}")
        if identifier in index:
            raise MachineReportError(f"duplicate {label} option ID: {identifier}")
        for field in OPTION_IDENTITY_FIELDS:
            if field not in case:
                raise MachineReportError(
                    f"{label} option {identifier} is missing {field}"
                )
        index[identifier] = case
    return index


def _case_outcome(
    case: Mapping[str, object],
    snapshot: Mapping[str, object],
    label: str,
) -> dict[str, object]:
    status = _required_string(case, "status", f"{label} option")
    outcome: dict[str, object] = {"status": status}
    for key in ("warnings", "logs"):
        if key not in case:
            raise MachineReportError(f"{label} option is missing {key}")
        outcome[key] = copy.deepcopy(case[key])
    if status == "ok":
        result = case.get("result")
        if not isinstance(result, dict) or set(result) != {"$ref"}:
            raise MachineReportError(f"{label} option result must contain one $ref")
        reference = result["$ref"]
        outputs = snapshot.get("outputs")
        if not isinstance(reference, str) or not isinstance(outputs, dict):
            raise MachineReportError(f"{label} option outputs are invalid")
        if re.fullmatch(r"[0-9a-f]{64}", reference) is None:
            raise MachineReportError(
                f"{label} option result reference is not a SHA-256 digest"
            )
        if reference not in outputs:
            raise MachineReportError(
                f"{label} option result references missing output {reference}"
            )
        canonical = json.dumps(
            outputs[reference],
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        if hashlib.sha256(canonical).hexdigest() != reference:
            raise MachineReportError(
                f"{label} option output does not match SHA-256 {reference}"
            )
        outcome["result_sha256"] = reference
        outcome["result"] = copy.deepcopy(outputs[reference])
    else:
        if "error" not in case:
            raise MachineReportError(f"{label} failed option is missing error")
        outcome["error"] = copy.deepcopy(case["error"])
    return outcome


def _validate_option_target(
    snapshot: Mapping[str, object],
    target: upstream.Target,
) -> None:
    if snapshot.get("schema_version") != 1:
        raise MachineReportError("unsupported option-matrix schema")
    raw_target = snapshot.get("target")
    if not isinstance(raw_target, dict):
        raise MachineReportError("option-matrix target must be a table")
    actual = (
        raw_target.get("project"),
        raw_target.get("version"),
        raw_target.get("commit"),
    )
    expected = (target.project, target.version, target.commit)
    if actual != expected:
        raise MachineReportError(
            f"option-matrix target {actual} does not match pinned target {expected}"
        )


def _page_sort_key(page: object) -> int:
    if not isinstance(page, dict):
        raise MachineReportError("page result must be a table")
    page_number = page.get("page_number")
    if isinstance(page_number, bool) or not isinstance(page_number, int):
        raise MachineReportError("page result has an invalid page number")
    return page_number


def _required_string(data: Mapping[str, object], key: str, context: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise MachineReportError(f"{context} has no non-empty {key}")
    return value
