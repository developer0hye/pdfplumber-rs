"""Fail-closed component timing plan for SCORE-004."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_competitors

REQUIRED_STAGE_IDS = (
    "document-open",
    "page-materialization",
    "character-extraction",
    "word-grouping",
    "table-detection",
    "serialization",
    "language-boundary-conversion",
)
ID_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
KNOWN_IMPLEMENTATIONS = frozenset(
    {
        "pdfplumber-python",
        "pdfplumber-rs",
        "pdfplumber-rs-python",
        "pdf-oxide",
        "pdfsink-rs",
    }
)


class BenchmarkStageError(ValueError):
    """A stage plan, semantic record, or sample is invalid."""


@dataclass(frozen=True)
class StageDefinition:
    """One semantic and clock boundary."""

    id: str
    output_schema: str
    fixture_ids: tuple[str, ...]
    semantic_reference: str
    semantic_implementations: tuple[str, ...]
    timed_implementations: tuple[str, ...]
    setup_operations: tuple[str, ...]
    timed_operation: str
    clock: str
    request: dict[str, object]


@dataclass(frozen=True)
class StageSuite:
    """Validated stage plan bound to the pinned competitor suite and corpus."""

    id: str
    release: str
    competitor_suite_id: str
    corpus_id: str
    semantic_reference: str
    candidate_python_id: str
    candidate_python_adapter: str
    stages: tuple[StageDefinition, ...]
    competitor_suite: benchmark_competitors.CompetitorSuite

    def stage(self, stage_id: str) -> StageDefinition | None:
        """Return one named stage."""

        for stage in self.stages:
            if stage.id == stage_id:
                return stage
        return None


def audit_repository(
    repo_root: Path,
    stages_path: Path,
    competitor_suite_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> StageSuite:
    """Load the stage plan and bind every identity to audited repository inputs."""

    competitor_suite = benchmark_competitors.audit_repository(
        repo_root,
        competitor_suite_path,
        corpus_path,
        policy_path,
        registry_path,
    )
    try:
        with stages_path.open("rb") as stages_file:
            source = tomllib.load(stages_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkStageError(f"cannot read stage plan: {stages_path}") from error
    return validate_suite(source, repo_root, competitor_suite)


def validate_suite(
    source: Mapping[str, object],
    repo_root: Path,
    competitor_suite: benchmark_competitors.CompetitorSuite,
) -> StageSuite:
    """Validate one parsed stage manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkStageError("schema.version must be 1")
    raw_suite = _required_mapping(source, "suite", "manifest")
    suite_id = _required_string(raw_suite, "id", "suite")
    release = _required_string(raw_suite, "release", "suite")
    competitor_suite_id = _required_string(
        raw_suite, "competitor_suite_id", "suite"
    )
    corpus_id = _required_string(raw_suite, "corpus_id", "suite")
    semantic_reference = _required_string(
        raw_suite, "semantic_reference", "suite"
    )
    candidate_python_id = _required_string(
        raw_suite, "candidate_python_id", "suite"
    )
    candidate_python_adapter = _required_repository_file(
        raw_suite,
        "candidate_python_adapter",
        "suite",
        repo_root,
    )
    if competitor_suite_id != competitor_suite.id:
        raise BenchmarkStageError("stage plan names the wrong competitor suite")
    if corpus_id != competitor_suite.corpus_id:
        raise BenchmarkStageError("stage plan names the wrong corpus")
    if release != competitor_suite.release:
        raise BenchmarkStageError("stage plan release differs from competitor suite")
    if semantic_reference != competitor_suite.reference_implementation:
        raise BenchmarkStageError("stage semantic reference differs from competitor suite")
    if candidate_python_id != "pdfplumber-rs-python":
        raise BenchmarkStageError("candidate Python id must be pdfplumber-rs-python")

    raw_stages = source.get("stages")
    if not isinstance(raw_stages, list) or not raw_stages:
        raise BenchmarkStageError("stages must be a non-empty array")
    stages = tuple(
        _validate_stage(raw_stage, semantic_reference, competitor_suite)
        for raw_stage in raw_stages
        if isinstance(raw_stage, dict)
    )
    if len(stages) != len(raw_stages):
        raise BenchmarkStageError("each stage must be one table")
    stage_ids = tuple(stage.id for stage in stages)
    if stage_ids != REQUIRED_STAGE_IDS:
        raise BenchmarkStageError(
            "stages must appear exactly as: " + ", ".join(REQUIRED_STAGE_IDS)
        )
    return StageSuite(
        id=suite_id,
        release=release,
        competitor_suite_id=competitor_suite_id,
        corpus_id=corpus_id,
        semantic_reference=semantic_reference,
        candidate_python_id=candidate_python_id,
        candidate_python_adapter=candidate_python_adapter,
        stages=stages,
        competitor_suite=competitor_suite,
    )


def _validate_stage(
    source: Mapping[str, object],
    semantic_reference: str,
    competitor_suite: benchmark_competitors.CompetitorSuite,
) -> StageDefinition:
    stage_id = _required_string(source, "id", "stage")
    if not ID_PATTERN.fullmatch(stage_id):
        raise BenchmarkStageError(f"invalid stage id: {stage_id}")
    output_schema = _required_string(source, "output_schema", f"stage {stage_id}")
    fixture_ids = _required_string_array(source, "fixture_ids", f"stage {stage_id}")
    known_fixture_ids = {fixture.id for fixture in competitor_suite.corpus.fixtures}
    unknown_fixtures = set(fixture_ids) - known_fixture_ids
    if unknown_fixtures:
        raise BenchmarkStageError(
            f"stage {stage_id} has unknown fixtures: "
            + ", ".join(sorted(unknown_fixtures))
        )
    semantic_implementations = _required_string_array(
        source,
        "semantic_implementations",
        f"stage {stage_id}",
    )
    timed_implementations = _required_string_array(
        source,
        "timed_implementations",
        f"stage {stage_id}",
    )
    unknown_implementations = (
        set(semantic_implementations) | set(timed_implementations)
    ) - KNOWN_IMPLEMENTATIONS
    if unknown_implementations:
        raise BenchmarkStageError(
            f"stage {stage_id} has unknown implementations: "
            + ", ".join(sorted(unknown_implementations))
        )
    if semantic_reference not in semantic_implementations:
        raise BenchmarkStageError(f"stage {stage_id} omits the semantic reference")
    if not set(timed_implementations).issubset(semantic_implementations):
        raise BenchmarkStageError(
            f"stage {stage_id} times an implementation without semantic output"
        )
    setup_operations = _required_string_array(
        source,
        "setup_operations",
        f"stage {stage_id}",
    )
    timed_operation = _required_string(
        source,
        "timed_operation",
        f"stage {stage_id}",
    )
    if timed_operation in setup_operations:
        raise BenchmarkStageError(f"stage {stage_id} times a setup operation")
    if any("process-launch" in value for value in (*setup_operations, timed_operation)):
        raise BenchmarkStageError(f"stage {stage_id} includes process launch")
    clock = _required_string(source, "clock", f"stage {stage_id}")
    if clock != "monotonic-wall":
        raise BenchmarkStageError(f"stage {stage_id} clock must be monotonic-wall")
    request = _required_mapping(source, "request", f"stage {stage_id}")
    _canonical_json(request)
    return StageDefinition(
        id=stage_id,
        output_schema=output_schema,
        fixture_ids=fixture_ids,
        semantic_reference=semantic_reference,
        semantic_implementations=semantic_implementations,
        timed_implementations=timed_implementations,
        setup_operations=setup_operations,
        timed_operation=timed_operation,
        clock=clock,
        request=dict(request),
    )


def synthetic_stage_record(
    *,
    stage: StageDefinition,
    implementation_id: str,
    revision: str,
    fixture_id: str,
    fixture_sha256: str,
    outcome: Mapping[str, object],
) -> dict[str, object]:
    """Build one untimed stage semantic record."""

    if implementation_id not in stage.semantic_implementations:
        raise BenchmarkStageError(
            f"implementation {implementation_id} is not semantic for {stage.id}"
        )
    return {
        "schema_version": 1,
        "implementation": {"id": implementation_id, "revision": revision},
        "fixture": {"id": fixture_id, "sha256": fixture_sha256},
        "stage": {"id": stage.id, "output_schema": stage.output_schema},
        "request": dict(stage.request),
        "outcome": dict(outcome),
    }


def build_timing_plan(
    stage: StageDefinition,
    records: Mapping[str, Mapping[str, object]],
) -> tuple[str, ...]:
    """Select only timed implementations with exact reference output."""

    reference = records.get(stage.semantic_reference)
    if not _successful_record(reference):
        return ()
    eligible: list[str] = []
    for implementation_id in stage.timed_implementations:
        record = records.get(implementation_id)
        if implementation_id == stage.semantic_reference:
            if _successful_record(record):
                eligible.append(implementation_id)
        elif record is not None and preflight(
            stage,
            reference,
            record,
        )["eligible_for_timing"]:
            eligible.append(implementation_id)
    return tuple(eligible)


def preflight(
    stage: StageDefinition,
    reference: Mapping[str, object],
    candidate: Mapping[str, object],
) -> dict[str, object]:
    """Compare one stage record without allowing a timed field into either side."""

    reasons: list[str] = []
    reference_implementation = _record_implementation_id(reference)
    candidate_implementation = _record_implementation_id(candidate)
    if reference_implementation != stage.semantic_reference:
        reasons.append("reference implementation differs from stage plan")
    if candidate_implementation not in stage.semantic_implementations:
        reasons.append("candidate implementation is not semantic for stage")
    for label, record in (("reference", reference), ("candidate", candidate)):
        stage_record = record.get("stage")
        if not isinstance(stage_record, dict) or stage_record != {
            "id": stage.id,
            "output_schema": stage.output_schema,
        }:
            reasons.append(f"{label} stage contract differs")
        if _canonical_json(record.get("request")) != _canonical_json(stage.request):
            reasons.append(f"{label} request semantics differ")
        outcome = record.get("outcome")
        if not isinstance(outcome, dict):
            reasons.append(f"{label} outcome is malformed")
        elif outcome.get("status") != "success":
            reasons.append(f"{label} outcome is {outcome.get('status')}")
    if _canonical_json(reference.get("fixture")) != _canonical_json(
        candidate.get("fixture")
    ):
        reasons.append("fixture identity differs")
    reference_digest = _successful_outcome_digest(reference.get("outcome"))
    candidate_digest = _successful_outcome_digest(candidate.get("outcome"))
    if (
        reference_digest is not None
        and candidate_digest is not None
        and reference_digest != candidate_digest
    ):
        reasons.append("canonical stage output differs")
    unique_reasons = list(dict.fromkeys(reasons))
    fixture = reference.get("fixture")
    fixture_id = fixture.get("id") if isinstance(fixture, dict) else "unknown"
    return {
        "schema_version": 1,
        "case_id": f"{fixture_id}:{stage.id}",
        "reference_implementation": reference_implementation,
        "implementation_id": candidate_implementation,
        "eligible_for_timing": not unique_reasons,
        "reasons": unique_reasons,
        "reference_output_sha256": reference_digest,
        "candidate_output_sha256": candidate_digest,
    }


def build_stage_sample(
    *,
    stage: StageDefinition,
    untimed_record: Mapping[str, object],
    measured_outcome: Mapping[str, object],
    wall_time_ns: int,
    command_argv: Sequence[str],
) -> dict[str, object]:
    """Bind one in-adapter clock to an unchanged untimed semantic outcome."""

    if isinstance(wall_time_ns, bool) or not isinstance(wall_time_ns, int):
        raise BenchmarkStageError("wall_time_ns must be an integer")
    if wall_time_ns <= 0:
        raise BenchmarkStageError("wall_time_ns must be positive")
    expected_outcome = untimed_record.get("outcome")
    if _canonical_json(expected_outcome) != _canonical_json(measured_outcome):
        raise BenchmarkStageError("timed output drifted from untimed stage output")
    if measured_outcome.get("status") != "success":
        raise BenchmarkStageError("only successful stage output can be timed")
    stage_record = untimed_record.get("stage")
    if not isinstance(stage_record, dict) or stage_record.get("id") != stage.id:
        raise BenchmarkStageError("untimed record names the wrong stage")
    implementation = untimed_record.get("implementation")
    fixture = untimed_record.get("fixture")
    if not isinstance(implementation, dict) or not isinstance(fixture, dict):
        raise BenchmarkStageError("untimed record lacks implementation or fixture")
    return {
        "schema_version": 1,
        "case_id": f"{fixture.get('id')}:{stage.id}",
        "stage_id": stage.id,
        "implementation": dict(implementation),
        "fixture": dict(fixture),
        "measurement_scope": "in-adapter-stage-only",
        "clock": stage.clock,
        "setup_operations": list(stage.setup_operations),
        "timed_operation": stage.timed_operation,
        "wall_time_ns": wall_time_ns,
        "semantic_output_sha256": _outcome_digest(measured_outcome),
        "command_argv": list(command_argv),
    }


def write_local_run(
    output_path: Path,
    *,
    records: Sequence[Mapping[str, object]],
    preflight_decisions: Sequence[Mapping[str, object]],
    stage_timings: Sequence[Mapping[str, object]],
) -> None:
    """Retain every semantic result and only eligible separated clocks."""

    payload = {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "records": list(records),
        "preflight_decisions": list(preflight_decisions),
        "stage_timings": list(stage_timings),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def render_markdown(suite: StageSuite) -> str:
    """Render the component boundaries without publishing measurements."""

    rows = [
        "| Stage | Fixtures | Semantic implementations | Timed implementations | Setup outside clock | Timed operation |",
        "|---|---:|---|---|---|---|",
    ]
    for stage in suite.stages:
        rows.append(
            "| "
            + " | ".join(
                (
                    f"`{stage.id}`",
                    str(len(stage.fixture_ids)),
                    ", ".join(f"`{value}`" for value in stage.semantic_implementations),
                    ", ".join(f"`{value}`" for value in stage.timed_implementations),
                    ", ".join(f"`{value}`" for value in stage.setup_operations),
                    f"`{stage.timed_operation}`",
                )
            )
            + " |"
        )
    return "\n".join(
        (
            f"# Separated benchmark stages {suite.release}",
            "",
            (
                f"Suite `{suite.id}` separates seven component clocks while retaining "
                f"corpus `{suite.corpus_id}` and pinned semantic reference "
                f"`{suite.semantic_reference}`."
            ),
            "",
            *rows,
            "",
            "## Validity boundary",
            "",
            (
                "Every adapter first emits an untimed canonical stage result. A clock is "
                "retained only when that result exactly matches the semantic reference, and "
                "the timed invocation reproduces the same result. Adapter process launch and "
                "the listed setup operations are outside the monotonic clock."
            ),
            "",
            (
                "The pinned APIs do not expose every cost as an independently comparable "
                "component. `pdfsink-rs` eagerly materializes pages and characters during "
                "document open, while the pinned `pdf_oxide` word and table entry points "
                "repeat extraction work. Those implementations still emit semantic outcomes "
                "but are excluded from the affected component clocks instead of being timed "
                "under a misleading label."
            ),
            "",
            (
                "Language-boundary conversion is candidate-specific: the installed PyO3 page "
                "cache is warmed outside the clock, then only native character to Python "
                "dictionary conversion is timed. Python `pdfplumber` supplies the untimed "
                "canonical output but has no equivalent native-language boundary clock."
            ),
            "",
            "```console",
            "python3 scripts/run_stage_benchmarks.py --check",
            (
                "python3 scripts/run_stage_benchmarks.py --run "
                "--output /tmp/pdfplumber-rs-stages.json"
            ),
            "```",
            "",
            (
                "SCORE-004 results remain local and unpublished. Wall time is the only "
                "component metric here; the separate SCORE-005 resource and artifact suite "
                "preserves that uninstrumented pass. Execution scenarios, environment "
                "metadata, repetitions, statistics, and release artifacts remain open under "
                "SCORE-006 through SCORE-008."
            ),
            "",
        )
    )


def _successful_record(record: Mapping[str, object] | None) -> bool:
    if record is None:
        return False
    outcome = record.get("outcome")
    return isinstance(outcome, dict) and outcome.get("status") == "success"


def _outcome_digest(outcome: object) -> str:
    return hashlib.sha256(_canonical_json(outcome).encode("utf-8")).hexdigest()


def _successful_outcome_digest(outcome: object) -> str | None:
    if not isinstance(outcome, dict) or outcome.get("status") != "success":
        return None
    return _outcome_digest(outcome)


def _record_implementation_id(record: Mapping[str, object]) -> str:
    implementation = record.get("implementation")
    if not isinstance(implementation, dict):
        return "unknown"
    implementation_id = implementation.get("id")
    return implementation_id if isinstance(implementation_id, str) else "unknown"


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
        raise BenchmarkStageError("stage value is not finite JSON") from error


def _required_mapping(
    source: Mapping[str, object], key: str, context: str
) -> dict[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise BenchmarkStageError(f"{context} needs one {key} table")
    return value


def _required_string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value:
        raise BenchmarkStageError(f"{context} needs a non-empty {key}")
    return value


def _required_string_array(
    source: Mapping[str, object], key: str, context: str
) -> tuple[str, ...]:
    value = source.get(key)
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item for item in value)
    ):
        raise BenchmarkStageError(f"{context} needs a non-empty {key} array")
    values = tuple(value)
    if len(set(values)) != len(values):
        raise BenchmarkStageError(f"{context} has duplicate {key} values")
    return values


def _required_repository_file(
    source: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    value = _required_string(source, key, context)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise BenchmarkStageError(f"{context} {key} is not repository-relative")
    if not (repo_root / path).is_file():
        raise BenchmarkStageError(f"{context} {key} is missing: {value}")
    return path.as_posix()
