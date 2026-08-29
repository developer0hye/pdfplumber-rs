"""Project machine compatibility observations into common user workflows."""

from __future__ import annotations

import copy
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from compat.harness import compatibility_scorecard

SCHEMA_VERSION: int = 1
GENERATOR: str = "scripts/generate_workflow_scorecard.py"
STATUS_ORDER: tuple[str, ...] = compatibility_scorecard.STATUSES
WORKFLOW_IDS: tuple[str, ...] = (
    "open",
    "text",
    "words",
    "crop",
    "search",
    "tables",
    "serialization",
    "annotations",
    "structure",
    "rendering",
    "cli",
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
KIND_ORDER: dict[str, int] = {"fixture": 0, "api": 1, "option": 2}
STATUS_LABELS: dict[str, str] = {
    "exact": "Exact",
    "approved_delta": "Approved delta",
    "unsupported": "Unsupported",
    "reference_failure": "Reference failure",
    "candidate_failure": "Candidate failure",
    "not_tested": "Not tested",
}


class WorkflowScorecardError(ValueError):
    """A machine observation cannot be projected without losing its meaning."""


@dataclass(frozen=True)
class WorkflowDefinition:
    """One ordered workflow and its machine-scorecard evidence selector."""

    identifier: str
    title: str
    api_ids: tuple[str, ...] = ()
    projection: str | None = None
    not_tested_reason: str | None = None


def build(
    machine_scorecard: Mapping[str, object],
    workflow_definitions: Sequence[WorkflowDefinition],
    *,
    machine_path: str,
    machine_sha256: str,
    indexed_fixture_ids: Sequence[str] | None = None,
) -> dict[str, object]:
    """Build a deterministic human projection without inventing parity evidence."""

    if machine_scorecard.get("schema_version") != compatibility_scorecard.SCHEMA_VERSION:
        raise WorkflowScorecardError("machine scorecard schema is unsupported")
    if not machine_path.strip():
        raise WorkflowScorecardError("machine scorecard path must not be empty")
    if SHA256.fullmatch(machine_sha256) is None:
        raise WorkflowScorecardError("machine scorecard SHA-256 is invalid")

    subject = _required_mapping(machine_scorecard, "subject", "machine scorecard")
    target = _required_mapping(machine_scorecard, "target", "machine scorecard")
    corpus = _required_mapping(machine_scorecard, "corpus", "machine scorecard")
    vocabulary = _required_mapping(
        machine_scorecard,
        "status_vocabulary",
        "machine scorecard",
    )
    if set(vocabulary) != set(STATUS_ORDER):
        raise WorkflowScorecardError("machine scorecard status vocabulary has drifted")

    dimensions = _required_mapping(
        machine_scorecard,
        "dimensions",
        "machine scorecard",
    )
    api_ids = _required_string_sequence(dimensions, "apis", "machine dimensions")
    if len(api_ids) != len(set(api_ids)):
        raise WorkflowScorecardError("machine scorecard API dimensions are duplicated")

    definitions = tuple(workflow_definitions)
    _validate_definitions(definitions, api_ids)
    observations = _observation_sequence(machine_scorecard)

    workflows: list[dict[str, object]] = []
    for definition in definitions:
        if definition.projection == "document_open":
            workflows.append(
                _document_open_workflow(
                    definition,
                    machine_scorecard,
                    observations,
                    indexed_fixture_ids,
                )
            )
        elif definition.not_tested_reason is not None:
            workflows.append(_not_tested_workflow(definition))
        else:
            workflows.append(_api_workflow(definition, observations))

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_by": GENERATOR,
        "machine_scorecard": {
            "path": machine_path,
            "sha256": machine_sha256,
            "schema_version": machine_scorecard["schema_version"],
        },
        "subject": copy.deepcopy(dict(subject)),
        "target": copy.deepcopy(dict(target)),
        "corpus": copy.deepcopy(dict(corpus)),
        "status_vocabulary": copy.deepcopy(dict(vocabulary)),
        "runs": _run_coverage(machine_scorecard, dimensions),
        "workflows": workflows,
    }


def render(report: Mapping[str, object]) -> str:
    """Render a compact Markdown view whose counts retain domain outcomes."""

    subject = _required_mapping(report, "subject", "workflow report")
    target = _required_mapping(report, "target", "workflow report")
    corpus = _required_mapping(report, "corpus", "workflow report")
    source = _required_mapping(report, "machine_scorecard", "workflow report")
    vocabulary = _required_mapping(
        report,
        "status_vocabulary",
        "workflow report",
    )
    workflows = _required_mapping_sequence(report, "workflows", "workflow report")
    runs = _required_mapping_sequence(report, "runs", "workflow report")

    machine_path = _required_string(source, "path", "machine scorecard")
    machine_link = machine_path.rsplit("/", maxsplit=1)[-1]
    subject_version = _required_string(subject, "version", "subject")
    subject_revision = _required_string(subject, "revision", "subject")
    target_version = _required_string(target, "version", "target")
    target_tag = _required_string(target, "tag", "target")
    corpus_count = corpus.get("fixture_count")
    if not isinstance(corpus_count, int) or corpus_count < 0:
        raise WorkflowScorecardError("corpus fixture_count must be non-negative")

    lines = [
        f"# Compatibility workflows for v{subject_version}",
        "",
        (
            "This human-readable view is generated from the versioned "
            f"[machine-readable scorecard]({machine_link}). It groups parity "
            "observations by user workflow; it is not a release-support or "
            "readiness claim."
        ),
        "",
        (
            "No success percentage is computed. Exact matches, approved deltas, "
            "unsupported behavior, reference failures, candidate failures, and "
            "untested coverage remain separate counts."
        ),
        (
            "The canonical [compatibility terminology](terms.md) defines the "
            "scope of compatible, extension, and approved-deviation claims."
        ),
        "",
        "## Provenance",
        "",
        f"- Candidate: `pdfplumber-rs` `{subject_version}` at `{subject_revision}`.",
        f"- Reference: `pdfplumber` `{target_version}` (`{target_tag}`).",
        f"- Indexed corpus: {corpus_count} PDFs.",
        (
            "- Machine scorecard SHA-256: "
            f"`{_required_string(source, 'sha256', 'machine scorecard')}`."
        ),
        "",
        "## Run coverage",
        "",
        (
            "Run coverage is reported separately from workflow outcomes. A package "
            "build or smoke test does not become a parity result."
        ),
        "",
        "| Platform and artifact | Scope | Coverage | Reason |",
        "| --- | --- | --- | --- |",
    ]
    for run in runs:
        scopes = _required_string_sequence(run, "scopes", "run coverage")
        status = _required_string(run, "status", "run coverage")
        reason = run.get("reason")
        reason_text = reason if isinstance(reason, str) and reason else "—"
        lines.append(
            "| "
            + " | ".join(
                (
                    _escape_table(_required_string(run, "label", "run coverage")),
                    _escape_table(", ".join(scopes)),
                    "Observed" if status == "observed" else "Not tested",
                    _escape_table(reason_text),
                )
            )
            + " |"
        )

    lines.extend(
        [
            "",
            "## Outcome vocabulary",
            "",
            "| Outcome | Meaning |",
            "| --- | --- |",
        ]
    )
    for status in STATUS_ORDER:
        meaning = vocabulary.get(status)
        if not isinstance(meaning, str) or not meaning:
            raise WorkflowScorecardError(f"missing vocabulary for {status}")
        lines.append(f"| {STATUS_LABELS[status]} | {_escape_table(meaning)} |")

    lines.extend(["", "## Workflows", ""])
    for workflow in workflows:
        title = _required_string(workflow, "title", "workflow")
        coverage = _required_string(workflow, "coverage", f"workflow {title}")
        lines.extend([f"### {title}", ""])
        if coverage == "not_tested":
            reason = _required_string(
                workflow,
                "not_tested_reason",
                f"workflow {title}",
            )
            lines.extend(
                [
                    f"Coverage: **Not tested** — {reason}",
                    "",
                    "Machine observations: 0.",
                    "",
                ]
            )
            continue

        api_values = _required_string_sequence(workflow, "api_ids", f"workflow {title}")
        projection = workflow.get("projection")
        if projection == "document_open":
            basis = (
                "one derived document outcome per indexed fixture from explicit "
                "fixture failures/gaps or the presence of page/API observations; "
                "an indexed fixture absent from both remains not tested"
            )
        else:
            basis = "API and option observations for " + ", ".join(
                f"`{value}`" for value in api_values
            )
        observation_count = workflow.get("observation_count")
        if not isinstance(observation_count, int) or observation_count < 0:
            raise WorkflowScorecardError(
                f"workflow {title} observation_count must be non-negative"
            )
        counts = _required_mapping(workflow, "status_counts", f"workflow {title}")
        lines.extend(
            [
                "Coverage: **Observed evidence**; this is not a workflow-level pass.",
                "",
                f"Evidence basis: {basis}.",
                "",
                f"Counted outcomes: {observation_count}.",
                "",
                "| Outcome | Count |",
                "| --- | ---: |",
            ]
        )
        for status in STATUS_ORDER:
            value = counts.get(status)
            if not isinstance(value, int) or value < 0:
                raise WorkflowScorecardError(
                    f"workflow {title} has an invalid {status} count"
                )
            lines.append(f"| {STATUS_LABELS[status]} | {value} |")
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def _validate_definitions(
    definitions: Sequence[WorkflowDefinition],
    machine_api_ids: Sequence[str],
) -> None:
    identifiers = tuple(definition.identifier for definition in definitions)
    if identifiers != WORKFLOW_IDS:
        raise WorkflowScorecardError(
            f"workflow IDs/order must be {WORKFLOW_IDS}, got {identifiers}"
        )

    mapped_api_ids: list[str] = []
    for definition in definitions:
        if not definition.title.strip():
            raise WorkflowScorecardError(
                f"workflow {definition.identifier} title must not be empty"
            )
        modes = sum(
            (
                bool(definition.api_ids),
                definition.projection is not None,
                definition.not_tested_reason is not None,
            )
        )
        if modes != 1:
            raise WorkflowScorecardError(
                f"workflow {definition.identifier} needs exactly one evidence mode"
            )
        if definition.projection not in {None, "document_open"}:
            raise WorkflowScorecardError(
                f"workflow {definition.identifier} has an unknown projection"
            )
        if definition.not_tested_reason is not None and not definition.not_tested_reason.strip():
            raise WorkflowScorecardError(
                f"workflow {definition.identifier} needs a not-tested reason"
            )
        mapped_api_ids.extend(definition.api_ids)

    if sorted(mapped_api_ids) != sorted(machine_api_ids) or len(mapped_api_ids) != len(
        set(mapped_api_ids)
    ):
        raise WorkflowScorecardError(
            "every machine API dimension must belong to exactly one workflow"
        )


def _api_workflow(
    definition: WorkflowDefinition,
    observations: Sequence[Mapping[str, object]],
) -> dict[str, object]:
    selected = [
        observation
        for observation in observations
        if observation.get("kind") in {"api", "option"}
        and observation.get("api_id") in definition.api_ids
    ]
    counts = _status_counts(selected)
    coverage = "observed" if selected else "not_tested"
    result: dict[str, object] = {
        "id": definition.identifier,
        "title": definition.title,
        "coverage": coverage,
        "api_ids": list(definition.api_ids),
        "evidence_kinds": _evidence_kinds(selected),
        "observation_count": len(selected),
        "status_counts": counts,
    }
    if not selected:
        result["not_tested_reason"] = (
            "Mapped machine scorecard dimensions have no parity observations."
        )
    return result


def _document_open_workflow(
    definition: WorkflowDefinition,
    machine_scorecard: Mapping[str, object],
    observations: Sequence[Mapping[str, object]],
    indexed_fixture_ids: Sequence[str] | None,
) -> dict[str, object]:
    runs = _required_mapping_sequence(machine_scorecard, "runs", "machine scorecard")
    api_run_ids = {
        run.get("id")
        for run in runs
        if run.get("status") == "observed"
        and isinstance(run.get("scopes"), list)
        and "api" in run["scopes"]
    }
    relevant = [
        observation
        for observation in observations
        if observation.get("kind") in {"fixture", "api"}
        and (
            observation.get("run_id") is None
            or observation.get("run_id") in api_run_ids
        )
        and isinstance(observation.get("fixture_id"), str)
    ]
    grouped: dict[tuple[object, str], list[Mapping[str, object]]] = {}
    for observation in relevant:
        fixture_id = observation["fixture_id"]
        if not isinstance(fixture_id, str):
            raise WorkflowScorecardError("open projection fixture ID is invalid")
        key = (observation.get("run_id"), fixture_id)
        grouped.setdefault(key, []).append(observation)

    corpus = _required_mapping(machine_scorecard, "corpus", "machine scorecard")
    fixture_count = corpus.get("fixture_count")
    if not isinstance(fixture_count, int) or fixture_count < 0:
        raise WorkflowScorecardError("machine corpus fixture_count is invalid")
    if indexed_fixture_ids is None:
        fixture_ids = tuple(sorted({key[1] for key in grouped}))
    else:
        fixture_ids = tuple(indexed_fixture_ids)
        if (
            len(fixture_ids) != fixture_count
            or len(fixture_ids) != len(set(fixture_ids))
            or not all(isinstance(value, str) and value for value in fixture_ids)
        ):
            raise WorkflowScorecardError(
                "indexed fixture IDs must match the machine corpus exactly"
            )

    has_run_identity = any(key[0] is not None for key in grouped)
    run_keys: tuple[object, ...]
    if has_run_identity:
        run_keys = tuple(sorted(api_run_ids))
    else:
        run_keys = (None,)

    unreported_fixture_count = 0
    for run_key in run_keys:
        for fixture_id in fixture_ids:
            key = (run_key, fixture_id)
            if key not in grouped:
                grouped[key] = []
                unreported_fixture_count += 1

    derived: list[dict[str, str]] = []
    for key in sorted(grouped, key=lambda value: (str(value[0]), value[1])):
        values = grouped[key]
        explicit = [value for value in values if value.get("kind") == "fixture"]
        if explicit:
            statuses = {value.get("status") for value in explicit}
            if len(statuses) != 1:
                raise WorkflowScorecardError(
                    f"fixture {key[1]} has conflicting open outcomes"
                )
            status = statuses.pop()
            if not isinstance(status, str):
                raise WorkflowScorecardError(
                    f"fixture {key[1]} has an invalid open outcome"
                )
        elif values:
            status = "exact"
        else:
            status = "not_tested"
        derived.append({"status": status})

    expected_count = fixture_count * len(run_keys)
    if len(derived) != expected_count:
        raise WorkflowScorecardError(
            "document-open projection does not cover every indexed fixture exactly once"
        )
    return {
        "id": definition.identifier,
        "title": definition.title,
        "coverage": "observed",
        "api_ids": [],
        "projection": "document_open",
        "evidence_kinds": _evidence_kinds(relevant),
        "observation_count": len(derived),
        "unreported_fixture_count": unreported_fixture_count,
        "status_counts": _status_counts(derived),
    }


def _not_tested_workflow(definition: WorkflowDefinition) -> dict[str, object]:
    return {
        "id": definition.identifier,
        "title": definition.title,
        "coverage": "not_tested",
        "api_ids": [],
        "evidence_kinds": [],
        "observation_count": 0,
        "status_counts": _empty_counts(),
        "not_tested_reason": definition.not_tested_reason,
    }


def _run_coverage(
    machine_scorecard: Mapping[str, object],
    dimensions: Mapping[str, object],
) -> list[dict[str, object]]:
    platform_values = dimensions.get("platforms", [])
    if not isinstance(platform_values, list):
        raise WorkflowScorecardError("machine platform dimensions must be an array")
    platform_labels: dict[str, str] = {}
    for value in platform_values:
        if not isinstance(value, Mapping):
            raise WorkflowScorecardError("machine platform dimension is invalid")
        platform_id = value.get("id")
        system = value.get("system")
        if isinstance(platform_id, str) and isinstance(system, str):
            platform_labels[platform_id] = system

    result: list[dict[str, object]] = []
    for run in _required_mapping_sequence(machine_scorecard, "runs", "machine scorecard"):
        run_id = _required_string(run, "id", "machine run")
        platform_id = _required_string(run, "platform_id", f"machine run {run_id}")
        artifact_type = _required_string(
            run,
            "artifact_type",
            f"machine run {run_id}",
        )
        platform = run.get("platform")
        fallback_label = None
        if isinstance(platform, Mapping):
            candidate = platform.get("label")
            if isinstance(candidate, str):
                fallback_label = candidate
        platform_label = platform_labels.get(platform_id, fallback_label or platform_id)
        status = _required_string(run, "status", f"machine run {run_id}")
        if status not in {"observed", "not_tested"}:
            raise WorkflowScorecardError(f"machine run {run_id} has an invalid status")
        scopes = _required_string_sequence(run, "scopes", f"machine run {run_id}")
        reason = run.get("reason")
        result.append(
            {
                "id": run_id,
                "label": f"{platform_label} {artifact_type}",
                "platform_id": platform_id,
                "artifact_type": artifact_type,
                "status": status,
                "scopes": scopes,
                **({"reason": reason} if isinstance(reason, str) else {}),
            }
        )
    return result


def _status_counts(
    observations: Sequence[Mapping[str, object]],
) -> dict[str, int]:
    counts = _empty_counts()
    for observation in observations:
        status = observation.get("status")
        if not isinstance(status, str) or status not in counts:
            raise WorkflowScorecardError(f"unknown machine observation status: {status!r}")
        counts[status] += 1
    return counts


def _empty_counts() -> dict[str, int]:
    return {status: 0 for status in STATUS_ORDER}


def _evidence_kinds(observations: Sequence[Mapping[str, object]]) -> list[str]:
    kinds = {
        kind
        for observation in observations
        if isinstance((kind := observation.get("kind")), str)
    }
    return sorted(kinds, key=lambda value: (KIND_ORDER.get(value, 99), value))


def _observation_sequence(
    machine_scorecard: Mapping[str, object],
) -> list[Mapping[str, object]]:
    values = machine_scorecard.get("observations")
    if not isinstance(values, list):
        raise WorkflowScorecardError("machine scorecard observations must be an array")
    result: list[Mapping[str, object]] = []
    for position, value in enumerate(values):
        if not isinstance(value, Mapping):
            raise WorkflowScorecardError(
                f"machine observation {position} must be an object"
            )
        result.append(value)
    return result


def _required_mapping_sequence(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> list[Mapping[str, object]]:
    values = source.get(key)
    if not isinstance(values, list):
        raise WorkflowScorecardError(f"{context} {key} must be an array")
    result: list[Mapping[str, object]] = []
    for position, value in enumerate(values):
        if not isinstance(value, Mapping):
            raise WorkflowScorecardError(
                f"{context} {key}[{position}] must be an object"
            )
        result.append(value)
    return result


def _required_mapping(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> Mapping[str, object]:
    value = source.get(key)
    if not isinstance(value, Mapping):
        raise WorkflowScorecardError(f"{context} {key} must be an object")
    return value


def _required_string(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value.strip():
        raise WorkflowScorecardError(f"{context} {key} must be a non-empty string")
    return value


def _required_string_sequence(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> list[str]:
    values = source.get(key)
    if not isinstance(values, list) or not all(
        isinstance(value, str) and value for value in values
    ):
        raise WorkflowScorecardError(
            f"{context} {key} must be an array of non-empty strings"
        )
    return list(values)


def _escape_table(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")
