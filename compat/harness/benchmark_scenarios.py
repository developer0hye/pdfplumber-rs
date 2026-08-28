"""Validated state, page-scope, and parallel benchmark scenarios."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_competitors

ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
REQUIRED_SCENARIOS = (
    "cold-document-open",
    "warm-document-open",
    "cache-hit-characters",
    "single-page-text",
    "full-document-text",
    "parallel-page-batch-text",
)
REFERENCE_IMPLEMENTATION = "pdfplumber-python"
CANDIDATE_PYTHON_ID = "pdfplumber-rs-python"


class BenchmarkScenarioError(ValueError):
    """The scenario plan or one local sample violates SCORE-006."""


@dataclass(frozen=True)
class ScenarioDefinition:
    """One explicit process, cache, page, and concurrency workload."""

    id: str
    output_schema: str
    fixture_ids: tuple[str, ...]
    semantic_implementations: tuple[str, ...]
    timed_implementations: tuple[str, ...]
    process_state: str
    cache_state: str
    page_selection: str
    setup_operations: tuple[str, ...]
    timed_operation: str
    clock: str
    concurrency_model: str
    worker_count: int
    output_order: str

    @property
    def request(self) -> dict[str, object]:
        """Return the canonical scenario request used for equivalence."""

        return {
            "cache_state": self.cache_state,
            "concurrency_model": self.concurrency_model,
            "output_order": self.output_order,
            "page_selection": self.page_selection,
            "process_state": self.process_state,
            "worker_count": self.worker_count,
        }


@dataclass(frozen=True)
class ScenarioSuite:
    """Scenario definitions bound to the pinned competitor suite."""

    id: str
    release: str
    competitor_suite_id: str
    candidate_python_id: str
    candidate_python_adapter: str
    filesystem_cache_control: str
    scenarios: tuple[ScenarioDefinition, ...]
    competitor_suite: benchmark_competitors.CompetitorSuite

    def scenario(self, scenario_id: str) -> ScenarioDefinition | None:
        """Return one scenario when present."""

        return next(
            (scenario for scenario in self.scenarios if scenario.id == scenario_id),
            None,
        )


def audit_repository(
    repo_root: Path,
    scenario_path: Path,
    competitor_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> ScenarioSuite:
    """Validate the scenario manifest and all referenced repository inputs."""

    competitor_suite = benchmark_competitors.audit_repository(
        repo_root,
        competitor_path,
        corpus_path,
        policy_path,
        registry_path,
    )
    try:
        with scenario_path.open("rb") as scenario_file:
            source = tomllib.load(scenario_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkScenarioError(
            f"cannot read scenario manifest: {scenario_path}"
        ) from error
    return validate_suite(source, repo_root, competitor_suite)


def validate_suite(
    source: Mapping[str, object],
    repo_root: Path,
    competitor_suite: benchmark_competitors.CompetitorSuite,
) -> ScenarioSuite:
    """Validate one parsed scenario manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkScenarioError("schema.version must be 1")
    raw_suite = _required_mapping(source, "suite", "manifest")
    suite_id = _required_string(raw_suite, "id", "suite")
    release = _required_string(raw_suite, "release", "suite")
    competitor_suite_id = _required_string(
        raw_suite, "competitor_suite_id", "suite"
    )
    candidate_python_id = _required_string(raw_suite, "candidate_python_id", "suite")
    candidate_python_adapter = _required_repository_file(
        raw_suite,
        "candidate_python_adapter",
        "suite",
        repo_root,
    )
    filesystem_cache_control = _required_string(
        raw_suite, "filesystem_cache_control", "suite"
    )
    if not ID_PATTERN.fullmatch(suite_id):
        raise BenchmarkScenarioError(f"invalid suite id: {suite_id}")
    if competitor_suite_id != competitor_suite.id:
        raise BenchmarkScenarioError("scenario suite names the wrong competitor suite")
    if release != competitor_suite.release:
        raise BenchmarkScenarioError("scenario and competitor releases must match")
    if candidate_python_id != CANDIDATE_PYTHON_ID:
        raise BenchmarkScenarioError(
            f"candidate_python_id must be {CANDIDATE_PYTHON_ID}"
        )
    if filesystem_cache_control != "uncontrolled-recorded":
        raise BenchmarkScenarioError(
            "filesystem cache must be recorded as uncontrolled"
        )

    raw_scenarios = source.get("scenarios")
    if not isinstance(raw_scenarios, list) or not raw_scenarios:
        raise BenchmarkScenarioError("scenarios must be a non-empty array")
    allowed_implementations = {
        implementation.id for implementation in competitor_suite.implementations
    } | {candidate_python_id}
    scenarios = tuple(
        _validate_scenario(raw, competitor_suite, allowed_implementations)
        for raw in raw_scenarios
        if isinstance(raw, dict)
    )
    if len(scenarios) != len(raw_scenarios):
        raise BenchmarkScenarioError("each scenario must be one table")
    if tuple(scenario.id for scenario in scenarios) != REQUIRED_SCENARIOS:
        raise BenchmarkScenarioError(
            f"scenario ids must be ordered as {REQUIRED_SCENARIOS}"
        )
    _validate_cross_scenario_contracts(scenarios)
    return ScenarioSuite(
        id=suite_id,
        release=release,
        competitor_suite_id=competitor_suite_id,
        candidate_python_id=candidate_python_id,
        candidate_python_adapter=candidate_python_adapter,
        filesystem_cache_control=filesystem_cache_control,
        scenarios=scenarios,
        competitor_suite=competitor_suite,
    )


def _validate_scenario(
    source: Mapping[str, object],
    competitor_suite: benchmark_competitors.CompetitorSuite,
    allowed_implementations: set[str],
) -> ScenarioDefinition:
    scenario_id = _required_string(source, "id", "scenario")
    if not ID_PATTERN.fullmatch(scenario_id):
        raise BenchmarkScenarioError(f"invalid scenario id: {scenario_id}")
    fixture_ids = _required_string_array(source, "fixture_ids", scenario_id)
    known_fixture_ids = {
        fixture.id for fixture in competitor_suite.corpus.fixtures
    }
    unknown_fixtures = set(fixture_ids) - known_fixture_ids
    if unknown_fixtures:
        raise BenchmarkScenarioError(
            f"{scenario_id} names unknown fixtures: {sorted(unknown_fixtures)}"
        )
    semantic_implementations = _required_string_array(
        source, "semantic_implementations", scenario_id
    )
    timed_implementations = _required_string_array(
        source, "timed_implementations", scenario_id
    )
    unknown_implementations = (
        set(semantic_implementations) | set(timed_implementations)
    ) - allowed_implementations
    if unknown_implementations:
        raise BenchmarkScenarioError(
            f"{scenario_id} names unknown implementations: "
            f"{sorted(unknown_implementations)}"
        )
    if REFERENCE_IMPLEMENTATION not in semantic_implementations:
        raise BenchmarkScenarioError(f"{scenario_id} lacks the semantic reference")
    if not set(timed_implementations).issubset(semantic_implementations):
        raise BenchmarkScenarioError(
            f"{scenario_id} times a non-semantic implementation"
        )
    worker_count = source.get("worker_count")
    if isinstance(worker_count, bool) or not isinstance(worker_count, int):
        raise BenchmarkScenarioError(f"{scenario_id}.worker_count must be an integer")
    if worker_count <= 0:
        raise BenchmarkScenarioError(f"{scenario_id}.worker_count must be positive")
    scenario = ScenarioDefinition(
        id=scenario_id,
        output_schema=_required_string(source, "output_schema", scenario_id),
        fixture_ids=fixture_ids,
        semantic_implementations=semantic_implementations,
        timed_implementations=timed_implementations,
        process_state=_required_string(source, "process_state", scenario_id),
        cache_state=_required_string(source, "cache_state", scenario_id),
        page_selection=_required_string(source, "page_selection", scenario_id),
        setup_operations=_required_string_array(
            source, "setup_operations", scenario_id
        ),
        timed_operation=_required_string(source, "timed_operation", scenario_id),
        clock=_required_string(source, "clock", scenario_id),
        concurrency_model=_required_string(
            source, "concurrency_model", scenario_id
        ),
        worker_count=worker_count,
        output_order=_required_string(source, "output_order", scenario_id),
    )
    if scenario.clock != "monotonic-wall":
        raise BenchmarkScenarioError(f"{scenario_id} must use monotonic-wall")
    if any("process-launch" in value for value in scenario.setup_operations):
        raise BenchmarkScenarioError(f"{scenario_id} must exclude process launch")
    return scenario


def _validate_cross_scenario_contracts(
    scenarios: tuple[ScenarioDefinition, ...],
) -> None:
    by_id = {scenario.id: scenario for scenario in scenarios}
    cold = by_id["cold-document-open"]
    warm = by_id["warm-document-open"]
    if cold.fixture_ids != warm.fixture_ids or cold.timed_operation != warm.timed_operation:
        raise BenchmarkScenarioError("cold and warm must differ only by declared state")
    cache_hit = by_id["cache-hit-characters"]
    if cache_hit.semantic_implementations != (
        REFERENCE_IMPLEMENTATION,
        CANDIDATE_PYTHON_ID,
    ):
        raise BenchmarkScenarioError("cache-hit must compare the two Python APIs")
    single = by_id["single-page-text"]
    full = by_id["full-document-text"]
    if single.fixture_ids != full.fixture_ids:
        raise BenchmarkScenarioError("single-page and full-document fixtures must match")
    if single.page_selection != "first-page" or full.page_selection != "all-pages":
        raise BenchmarkScenarioError("page scopes are not explicit")
    for scenario in (single, full):
        if (
            "pdfsink-rs" not in scenario.semantic_implementations
            or "pdfsink-rs" in scenario.timed_implementations
        ):
            raise BenchmarkScenarioError(
                "eager pdfsink-rs page content must remain semantic-only"
            )
    parallel = by_id["parallel-page-batch-text"]
    if (
        parallel.concurrency_model != "bounded-rayon-thread-pool"
        or parallel.worker_count != 4
        or parallel.timed_implementations != ("pdfplumber-rs",)
        or parallel.output_order != "page-index-order"
    ):
        raise BenchmarkScenarioError("parallel workload must be bounded and ordered")


def synthetic_scenario_record(
    *,
    scenario: ScenarioDefinition,
    implementation_id: str,
    revision: str,
    fixtures: Sequence[tuple[str, str]],
    outcome: Mapping[str, object],
) -> dict[str, object]:
    """Build one untimed canonical scenario record."""

    return {
        "schema_version": 1,
        "implementation": {"id": implementation_id, "revision": revision},
        "fixtures": [
            {"id": fixture_id, "sha256": fixture_sha256}
            for fixture_id, fixture_sha256 in fixtures
        ],
        "scenario": {"id": scenario.id, "output_schema": scenario.output_schema},
        "request": scenario.request,
        "outcome": dict(outcome),
    }


def preflight(
    scenario: ScenarioDefinition,
    reference: Mapping[str, object],
    candidate: Mapping[str, object],
) -> dict[str, object]:
    """Require identical scenario, fixture, request, and successful output."""

    reasons: list[str] = []
    reference_id = _implementation_id(reference)
    candidate_id = _implementation_id(candidate)
    if reference_id != REFERENCE_IMPLEMENTATION:
        reasons.append("reference implementation differs from scenario plan")
    if candidate_id not in scenario.semantic_implementations:
        reasons.append("candidate implementation is not semantic for scenario")
    expected_scenario = {"id": scenario.id, "output_schema": scenario.output_schema}
    for label, record in (("reference", reference), ("candidate", candidate)):
        if record.get("scenario") != expected_scenario:
            reasons.append(f"{label} scenario contract differs")
        if _canonical_json(record.get("request")) != _canonical_json(scenario.request):
            reasons.append(f"{label} request semantics differ")
        outcome = record.get("outcome")
        if not isinstance(outcome, dict):
            reasons.append(f"{label} outcome is malformed")
        elif outcome.get("status") != "success":
            reasons.append(f"{label} outcome is {outcome.get('status')}")
    if _canonical_json(reference.get("fixtures")) != _canonical_json(
        candidate.get("fixtures")
    ):
        reasons.append("fixture identity differs")
    reference_digest = _successful_outcome_digest(reference.get("outcome"))
    candidate_digest = _successful_outcome_digest(candidate.get("outcome"))
    if (
        reference_digest is not None
        and candidate_digest is not None
        and reference_digest != candidate_digest
    ):
        reasons.append("candidate output differs from reference")
    unique_reasons = list(dict.fromkeys(reasons))
    fixtures = reference.get("fixtures")
    fixture_id = "unknown"
    if isinstance(fixtures, list) and fixtures and isinstance(fixtures[0], dict):
        fixture_id = str(fixtures[0].get("id", fixture_id))
    return {
        "schema_version": 1,
        "case_id": f"{fixture_id}:{scenario.id}",
        "scenario_id": scenario.id,
        "reference_implementation": reference_id,
        "implementation_id": candidate_id,
        "eligible_for_timing": not unique_reasons,
        "reasons": unique_reasons,
        "reference_output_sha256": reference_digest,
        "candidate_output_sha256": candidate_digest,
    }


def build_scenario_sample(
    *,
    scenario: ScenarioDefinition,
    untimed_record: Mapping[str, object],
    measured_outcome: Mapping[str, object],
    wall_time_ns: int,
    command_argv: Sequence[str],
) -> dict[str, object]:
    """Bind one scenario clock to unchanged untimed output."""

    if isinstance(wall_time_ns, bool) or not isinstance(wall_time_ns, int):
        raise BenchmarkScenarioError("wall_time_ns must be an integer")
    if wall_time_ns <= 0:
        raise BenchmarkScenarioError("wall_time_ns must be positive")
    if _canonical_json(untimed_record.get("outcome")) != _canonical_json(
        measured_outcome
    ):
        raise BenchmarkScenarioError(
            "timed scenario output drifted from untimed scenario output"
        )
    if measured_outcome.get("status") != "success":
        raise BenchmarkScenarioError("only successful scenario output can be timed")
    scenario_record = untimed_record.get("scenario")
    if not isinstance(scenario_record, dict) or scenario_record.get("id") != scenario.id:
        raise BenchmarkScenarioError("untimed record names the wrong scenario")
    implementation = untimed_record.get("implementation")
    fixtures = untimed_record.get("fixtures")
    if not isinstance(implementation, dict) or not isinstance(fixtures, list):
        raise BenchmarkScenarioError("untimed record lacks implementation or fixtures")
    fixture_id = fixtures[0].get("id") if fixtures and isinstance(fixtures[0], dict) else "unknown"
    return {
        "schema_version": 1,
        "case_id": f"{fixture_id}:{scenario.id}",
        "scenario_id": scenario.id,
        "implementation": dict(implementation),
        "fixtures": list(fixtures),
        "measurement_scope": "in-adapter-scenario-only",
        "process_state": scenario.process_state,
        "cache_state": scenario.cache_state,
        "page_selection": scenario.page_selection,
        "concurrency_model": scenario.concurrency_model,
        "worker_count": scenario.worker_count,
        "output_order": scenario.output_order,
        "clock": scenario.clock,
        "setup_operations": list(scenario.setup_operations),
        "timed_operation": scenario.timed_operation,
        "wall_time_ns": wall_time_ns,
        "semantic_output_sha256": _outcome_digest(measured_outcome),
        "command_argv": list(command_argv),
    }


def write_local_run(
    output_path: Path,
    *,
    records: Sequence[Mapping[str, object]],
    preflight_decisions: Sequence[Mapping[str, object]],
    scenario_timings: Sequence[Mapping[str, object]],
) -> None:
    """Retain semantic rejections separately from eligible scenario clocks."""

    payload = {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "records": list(records),
        "preflight_decisions": list(preflight_decisions),
        "scenario_timings": list(scenario_timings),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def render_markdown(suite: ScenarioSuite) -> str:
    """Render scenario boundaries without publishing measurements."""

    rows = [
        "| Scenario | Fixtures | State | Page scope | Concurrency | Timed implementations | Timed operation |",
        "|---|---|---|---|---|---|---|",
    ]
    for scenario in suite.scenarios:
        rows.append(
            "| "
            + " | ".join(
                (
                    f"`{scenario.id}`",
                    ", ".join(f"`{value}`" for value in scenario.fixture_ids),
                    f"`{scenario.process_state}` / `{scenario.cache_state}`",
                    f"`{scenario.page_selection}`",
                    f"`{scenario.concurrency_model}` ({scenario.worker_count})",
                    ", ".join(
                        f"`{value}`" for value in scenario.timed_implementations
                    ),
                    f"`{scenario.timed_operation}`",
                )
            )
            + " |"
        )
    return "\n".join(
        (
            "# Benchmark Workload Scenarios v0.3.0",
            "",
            "SCORE-006 distinguishes process and library-cache state, page scope, and bounded parallel page work before repetitions or statistical summaries are added.",
            "",
            *rows,
            "",
            "## State boundary",
            "",
            "`cold-document-open` starts in a fresh adapter process with empty library state. It does not claim a cold operating-system page cache; filesystem cache state is `uncontrolled-recorded`. `warm-document-open` performs and closes one identical open in the same process before the clock. `cache-hit-characters` times the second identical character-property access on the same live page.",
            "",
            "## Equivalence and timing",
            "",
            "Every implementation first emits an untimed canonical result. Timing is allowed only when that exact fixture, request, and output match pinned Python `pdfplumber`; timed invocations must reproduce the same output. Process launch and all listed setup operations remain outside the clock.",
            "",
            "Pinned `pdfsink-rs` remains a semantic participant for single-page and full-document text but is not timed there because its document-open API eagerly materializes page content. Assigning its later extraction access the same lazy post-open scope would be misleading.",
            "",
            "The parallel scenario uses a four-worker Rayon pool and preserves page-index order. Pinned Python `pdfplumber` provides its untimed semantic reference, while only the parallel Rust implementation is clocked.",
            "",
            "```console",
            "python3 scripts/run_benchmark_scenarios.py --check",
            "python3 scripts/run_benchmark_scenarios.py --run --output /tmp/pdfplumber-rs-scenarios.json",
            "```",
            "",
            "SCORE-006 results remain local and unpublished. Complete environment capture, repetitions, statistical summaries, retained release artifacts, and result-removal policy remain open under SCORE-007 through SCORE-009.",
            "",
        )
    )


def _required_mapping(
    source: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise BenchmarkScenarioError(f"{context}.{key} must be a table")
    return value


def _required_string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value.strip():
        raise BenchmarkScenarioError(f"{context}.{key} must be a non-empty string")
    return value


def _required_string_array(
    source: Mapping[str, object], key: str, context: str
) -> tuple[str, ...]:
    value = source.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise BenchmarkScenarioError(f"{context}.{key} must be a string array")
    result = tuple(value)
    if len(set(result)) != len(result):
        raise BenchmarkScenarioError(f"{context}.{key} must be unique")
    return result


def _required_repository_file(
    source: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    value = _required_string(source, key, context)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not (repo_root / path).is_file():
        raise BenchmarkScenarioError(f"{context}.{key} must name a repository file")
    return path.as_posix()


def _implementation_id(record: Mapping[str, object]) -> str:
    implementation = record.get("implementation")
    if not isinstance(implementation, dict):
        return "unknown"
    value = implementation.get("id")
    return value if isinstance(value, str) else "unknown"


def _canonical_json(value: object) -> str:
    return json.dumps(value, allow_nan=False, ensure_ascii=False, sort_keys=True)


def _successful_outcome_digest(outcome: object) -> str | None:
    if not isinstance(outcome, dict) or outcome.get("status") != "success":
        return None
    return _outcome_digest(outcome)


def _outcome_digest(outcome: Mapping[str, object]) -> str:
    return hashlib.sha256(_canonical_json(outcome.get("value")).encode()).hexdigest()
