"""Deterministic human-readable rendering of machine parity reports."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any

from compat.harness.machine_report import PAGE_APIS, SCHEMA_VERSION


COORDINATE_FIELDS: tuple[str, ...] = (
    "x0",
    "x1",
    "y0",
    "y1",
    "top",
    "bottom",
    "doctop",
    "width",
    "height",
)


class HumanSummaryError(RuntimeError):
    """A machine report cannot be summarized without ambiguous evidence."""


def render(report: Mapping[str, object]) -> str:
    """Render one stable Markdown summary and its first differing result."""
    if report.get("schema_version") != SCHEMA_VERSION:
        raise HumanSummaryError("unsupported machine-report schema")
    target = _mapping(report.get("target"), "target")
    summary = _mapping(report.get("summary"), "summary")
    delta_gate = _mapping(report.get("approved_delta_gate"), "approved delta gate")
    status = _required_string(report, "status", "report")

    lines = [
        "# pdfplumber-rs parity summary",
        "",
        f"- Status: **{status.upper()}**",
        "- Target: "
        f"`{target.get('project')} {target.get('version')}` "
        f"(`{target.get('commit')}`)",
        f"- Fixtures: {_integer(summary, 'fixtures_total')} total, "
        f"{_integer(summary, 'fixtures_failed')} failed",
        f"- Pages compared: {_integer(summary, 'pages_compared')}",
        "- Page API results: "
        f"{_integer(summary, 'api_results_equal')} equal, "
        f"{_integer(summary, 'api_results_different')} different, "
        f"{_integer(summary, 'api_results_unsupported')} unsupported",
        "- Option cases: "
        f"{_integer(summary, 'option_cases_total')} total, "
        f"{_integer(summary, 'option_cases_equal')} equal, "
        f"{_integer(summary, 'option_cases_different')} different, "
        f"{_integer(summary, 'option_cases_blocked')} blocked",
        "- Approved delta gate: "
        f"{_integer(delta_gate, 'approved')} approved, "
        f"{_integer(delta_gate, 'unregistered')} unregistered, "
        f"{_integer(delta_gate, 'stale')} stale",
        "",
        "## First differing result",
        "",
    ]
    first = _first_differing_result(report)
    if first is None:
        lines.append("No differing page-API result was recorded.")
    else:
        lines.extend(_difference_lines(*first))
    return "\n".join(lines) + "\n"


def _first_differing_result(
    report: Mapping[str, object],
) -> tuple[str, int, str, Mapping[str, object], Mapping[str, object]] | None:
    fixtures = report.get("fixtures")
    if not isinstance(fixtures, list):
        raise HumanSummaryError("fixtures must be a list")
    fixture_records = sorted(
        (_mapping(item, "fixture") for item in fixtures),
        key=lambda item: _required_string(item, "fixture_id", "fixture"),
    )
    api_order = {api: index for index, api in enumerate(PAGE_APIS)}
    for fixture in fixture_records:
        if fixture.get("status") != "compared":
            continue
        fixture_id = _required_string(fixture, "fixture_id", "fixture")
        pages = fixture.get("pages")
        if not isinstance(pages, list):
            raise HumanSummaryError(f"{fixture_id}.pages must be a list")
        for page in sorted(
            (_mapping(item, f"{fixture_id} page") for item in pages),
            key=lambda item: _integer(item, "page_number"),
        ):
            page_number = _integer(page, "page_number")
            if page.get("status") != "compared":
                continue
            apis = _mapping(page.get("apis"), f"{fixture_id} page {page_number} APIs")
            for api in sorted(
                apis,
                key=lambda name: (api_order.get(str(name), len(api_order)), str(name)),
            ):
                outcome = _mapping(apis[api], f"{fixture_id} page {page_number} {api}")
                if outcome.get("status") != "different":
                    continue
                comparison = _mapping(outcome.get("comparison"), f"{api} comparison")
                difference = _mapping(
                    comparison.get("first_difference"),
                    f"{api} first difference",
                )
                return fixture_id, page_number, str(api), outcome, difference
    return None


def _difference_lines(
    fixture_id: str,
    page_number: int,
    api: str,
    outcome: Mapping[str, object],
    difference: Mapping[str, object],
) -> list[str]:
    kind = _required_string(difference, "kind", f"{api} first difference")
    upstream = difference.get("upstream")
    rust = difference.get("rust")
    lines = [
        f"- Fixture: `{_code(fixture_id)}`",
        f"- Page: {page_number}",
        f"- API: `{_code(api)}`",
    ]
    if kind == "sequence":
        lines.append(f"- Object index: {_integer(difference, 'index')}")
    elif kind == "text":
        lines.append(f"- Text offset: {_integer(difference, 'index')}")
    elif kind == "value":
        lines.append("- Value location: root")
    else:
        raise HumanSummaryError(f"unknown first-difference kind: {kind!r}")

    upstream_text, rust_text = _text_values(difference, upstream, rust)
    if upstream_text is not None or rust_text is not None:
        lines.append(
            f"- Text: upstream `{_inline_json(upstream_text)}` "
            f"-> Rust `{_inline_json(rust_text)}`"
        )

    coordinates = _coordinate_differences(upstream, rust)
    if coordinates:
        lines.append("- Coordinates: " + "; ".join(coordinates))
    elif _has_common_coordinates(upstream, rust):
        lines.append("- Coordinates: no differing common coordinates")

    delta = outcome.get("delta_gate")
    if isinstance(delta, Mapping):
        lines.append(
            "- Delta gate: "
            f"`{_code(_required_string(delta, 'status', 'delta gate'))}`"
        )

    if kind in {"sequence", "value"}:
        lines.extend(
            [
                f"- Upstream object: `{_inline_json(upstream)}`",
                f"- Rust object: `{_inline_json(rust)}`",
            ]
        )
    return lines


def _text_values(
    difference: Mapping[str, object],
    upstream: object,
    rust: object,
) -> tuple[object | None, object | None]:
    if difference.get("kind") == "text":
        return difference.get("upstream_context"), difference.get("rust_context")
    if isinstance(upstream, Mapping) or isinstance(rust, Mapping):
        left = upstream.get("text") if isinstance(upstream, Mapping) else None
        right = rust.get("text") if isinstance(rust, Mapping) else None
        return left, right
    if isinstance(upstream, str) or isinstance(rust, str):
        return upstream, rust
    return None, None


def _coordinate_differences(upstream: object, rust: object) -> list[str]:
    if not isinstance(upstream, Mapping) or not isinstance(rust, Mapping):
        return []
    differences: list[str] = []
    for field in COORDINATE_FIELDS:
        left = upstream.get(field)
        right = rust.get(field)
        if (
            isinstance(left, (int, float))
            and not isinstance(left, bool)
            and isinstance(right, (int, float))
            and not isinstance(right, bool)
            and left != right
        ):
            delta = float(right) - float(left)
            differences.append(
                f"{field} `{_inline_json(left)}` -> `{_inline_json(right)}` "
                f"(delta `{delta:+.12g}`)"
            )
    return differences


def _has_common_coordinates(upstream: object, rust: object) -> bool:
    if not isinstance(upstream, Mapping) or not isinstance(rust, Mapping):
        return False
    return any(
        isinstance(upstream.get(field), (int, float))
        and not isinstance(upstream.get(field), bool)
        and isinstance(rust.get(field), (int, float))
        and not isinstance(rust.get(field), bool)
        for field in COORDINATE_FIELDS
    )


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise HumanSummaryError(f"{context} must be a table")
    return value


def _integer(data: Mapping[str, object], key: str) -> int:
    value = data.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        raise HumanSummaryError(f"{key} must be an integer")
    return value


def _required_string(data: Mapping[str, object], key: str, context: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise HumanSummaryError(f"{context} has no non-empty {key}")
    return value


def _inline_json(value: object) -> str:
    return _code(
        json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    )


def _code(value: str) -> str:
    return value.replace("`", "\\u0060")
