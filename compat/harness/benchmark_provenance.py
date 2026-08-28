"""Benchmark environment, repetition, and statistical provenance (SCORE-007)."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import statistics
import subprocess
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

import tomllib

from compat.harness import benchmark_scenarios

ID_PATTERN = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REPOSITORY = "https://github.com/developer0hye/pdfplumber-rs"
REQUIRED_STATISTICS = (
    "sample-size",
    "minimum",
    "median",
    "arithmetic-mean",
    "maximum",
    "sample-standard-deviation",
    "relative-standard-deviation",
)
REQUIRED_LOCKS = {
    "rust-workspace": "Cargo.lock",
    "rust-benchmark-adapter": "benchmarks/adapters/rust/Cargo.lock",
    "python-reference": "compat/requirements-golden.txt",
}
REQUIRED_BUILDS = {
    "rust-benchmark-adapter",
    "candidate-python-wheel",
}
REQUIRED_TOOLS = {
    "harness-python",
    "reference-python",
    "candidate-python",
    "rustc",
    "cargo",
    "maturin",
}
REQUIRED_ARTIFACTS = {
    "rust-benchmark-adapter",
    "candidate-python-native-extension",
}


class BenchmarkProvenanceError(ValueError):
    """Raised when benchmark run provenance is incomplete or inconsistent."""


@dataclass(frozen=True)
class DependencyLock:
    """One dependency lock that is hashed into every run."""

    role: str
    path: str


@dataclass(frozen=True)
class BuildDefinition:
    """One reproducible benchmark artifact build and its material flags."""

    id: str
    command_argv: tuple[str, ...]
    flags: tuple[str, ...]


@dataclass(frozen=True)
class ToolDefinition:
    """One compiler, interpreter, or build-tool version command."""

    id: str
    command_argv: tuple[str, ...]


@dataclass(frozen=True)
class ProvenancePlan:
    """Validated environment and repetition policy for the scenario suite."""

    id: str
    release: str
    scenario_suite_id: str
    repetitions: int
    execution_order: str
    working_tree_policy: str
    statistics: tuple[str, ...]
    reproduction_command: tuple[str, ...]
    dependency_locks: tuple[DependencyLock, ...]
    builds: tuple[BuildDefinition, ...]
    tools: tuple[ToolDefinition, ...]
    scenario_suite: benchmark_scenarios.ScenarioSuite


def audit_repository(
    repo_root: Path,
    provenance_path: Path,
    scenario_path: Path,
    competitor_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> ProvenancePlan:
    """Validate the committed provenance plan and all referenced inputs."""

    scenario_suite = benchmark_scenarios.audit_repository(
        repo_root,
        scenario_path,
        competitor_path,
        corpus_path,
        policy_path,
        registry_path,
    )
    try:
        with provenance_path.open("rb") as provenance_file:
            source = tomllib.load(provenance_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkProvenanceError(
            f"cannot read provenance manifest: {provenance_path}"
        ) from error
    return validate_plan(source, repo_root, scenario_suite)


def validate_plan(
    source: Mapping[str, object],
    repo_root: Path,
    scenario_suite: benchmark_scenarios.ScenarioSuite,
) -> ProvenancePlan:
    """Validate one parsed provenance manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkProvenanceError("schema.version must be 1")
    raw_suite = _required_mapping(source, "suite", "manifest")
    plan_id = _required_string(raw_suite, "id", "suite")
    if not ID_PATTERN.fullmatch(plan_id):
        raise BenchmarkProvenanceError(f"invalid provenance id: {plan_id}")
    release = _required_string(raw_suite, "release", "suite")
    scenario_suite_id = _required_string(raw_suite, "scenario_suite_id", "suite")
    if release != scenario_suite.release:
        raise BenchmarkProvenanceError("provenance and scenario releases must match")
    if scenario_suite_id != scenario_suite.id:
        raise BenchmarkProvenanceError("provenance names the wrong scenario suite")
    repetitions = raw_suite.get("repetitions")
    if isinstance(repetitions, bool) or not isinstance(repetitions, int):
        raise BenchmarkProvenanceError("suite.repetitions must be an integer")
    if repetitions < 5:
        raise BenchmarkProvenanceError("suite.repetitions must be at least 5")
    execution_order = _required_string(raw_suite, "execution_order", "suite")
    if execution_order != "round-robin-by-repetition":
        raise BenchmarkProvenanceError(
            "suite.execution_order must be round-robin-by-repetition"
        )
    working_tree_policy = _required_string(raw_suite, "working_tree_policy", "suite")
    if working_tree_policy != "require-clean":
        raise BenchmarkProvenanceError("suite.working_tree_policy must require clean")
    statistics_names = _required_string_array(raw_suite, "statistics", "suite")
    if statistics_names != REQUIRED_STATISTICS:
        raise BenchmarkProvenanceError(
            f"suite.statistics must be ordered as {REQUIRED_STATISTICS}"
        )
    reproduction_command = _required_string_array(
        raw_suite, "reproduction_command", "suite"
    )
    if reproduction_command != (
        "python3",
        "scripts/run_benchmark_provenance.py",
        "--run",
        "--output",
        "<output-json>",
    ):
        raise BenchmarkProvenanceError("suite.reproduction_command is not canonical")

    dependency_locks = _validate_dependency_locks(
        source.get("dependency_locks"), repo_root
    )
    builds = _validate_builds(source.get("builds"))
    tools = _validate_tools(source.get("tools"))
    return ProvenancePlan(
        id=plan_id,
        release=release,
        scenario_suite_id=scenario_suite_id,
        repetitions=repetitions,
        execution_order=execution_order,
        working_tree_policy=working_tree_policy,
        statistics=statistics_names,
        reproduction_command=reproduction_command,
        dependency_locks=dependency_locks,
        builds=builds,
        tools=tools,
        scenario_suite=scenario_suite,
    )


def summarize_samples(
    samples: Sequence[Mapping[str, object]], *, repetitions: int
) -> list[dict[str, object]]:
    """Summarize complete, output-stable repetition groups without dropping samples."""

    if isinstance(repetitions, bool) or not isinstance(repetitions, int):
        raise BenchmarkProvenanceError("repetitions must be an integer")
    if repetitions < 2:
        raise BenchmarkProvenanceError("repetitions must be at least 2")
    groups: dict[tuple[str, str, str], list[Mapping[str, object]]] = {}
    for item in samples:
        case_id = _mapping_string(item, "case_id", "sample")
        scenario_id = _mapping_string(item, "scenario_id", "sample")
        implementation = _required_mapping(item, "implementation", "sample")
        implementation_id = _mapping_string(
            implementation, "id", "sample implementation"
        )
        groups.setdefault((scenario_id, case_id, implementation_id), []).append(item)

    summaries: list[dict[str, object]] = []
    expected_repetitions = list(range(1, repetitions + 1))
    for group_key in sorted(groups):
        group = groups[group_key]
        ordered = sorted(group, key=_sample_repetition)
        actual_repetitions = [_sample_repetition(item) for item in ordered]
        if actual_repetitions != expected_repetitions:
            raise BenchmarkProvenanceError(
                f"{group_key} repetitions are {actual_repetitions}, "
                f"expected {expected_repetitions}"
            )
        stable = _sample_stable_projection(ordered[0])
        for item in ordered[1:]:
            if _canonical_json(_sample_stable_projection(item)) != _canonical_json(
                stable
            ):
                raise BenchmarkProvenanceError(
                    f"{group_key} changed identity, command, or semantic output"
                )
        wall_times = [_sample_wall_time(item) for item in ordered]
        mean = statistics.fmean(wall_times)
        deviation = statistics.stdev(wall_times)
        summaries.append(
            {
                "schema_version": 1,
                "case_id": stable["case_id"],
                "scenario_id": stable["scenario_id"],
                "implementation": stable["implementation"],
                "fixtures": stable["fixtures"],
                "measurement_scope": stable["measurement_scope"],
                "clock": stable["clock"],
                "timed_operation": stable["timed_operation"],
                "semantic_output_sha256": stable["semantic_output_sha256"],
                "command_argv": stable["command_argv"],
                "sample_size": len(ordered),
                "repetitions": actual_repetitions,
                "minimum_wall_time_ns": min(wall_times),
                "median_wall_time_ns": statistics.median(wall_times),
                "arithmetic_mean_wall_time_ns": mean,
                "maximum_wall_time_ns": max(wall_times),
                "sample_standard_deviation_wall_time_ns": deviation,
                "relative_standard_deviation": deviation / mean,
                "samples_sha256": hashlib.sha256(
                    _canonical_json(ordered).encode("utf-8")
                ).hexdigest(),
            }
        )
    return summaries


def validate_run_metadata(metadata: Mapping[str, object], *, repetitions: int) -> None:
    """Reject a run unless every SCORE-007 provenance field is complete."""

    recorded_at = _mapping_string(metadata, "recorded_at_utc", "run metadata")
    if not recorded_at.endswith("Z"):
        raise BenchmarkProvenanceError("recorded_at_utc must be UTC with a Z suffix")

    source = _required_mapping(metadata, "source", "run metadata")
    if _mapping_string(source, "repository", "source") != REPOSITORY:
        raise BenchmarkProvenanceError("source.repository is not canonical")
    if not SHA_PATTERN.fullmatch(_mapping_string(source, "revision", "source")):
        raise BenchmarkProvenanceError("source.revision must be a full Git SHA")
    if source.get("working_tree_clean") is not True:
        raise BenchmarkProvenanceError("source.working_tree_clean must be true")

    host = _required_mapping(metadata, "host", "run metadata")
    for key in (
        "operating_system",
        "operating_system_release",
        "architecture",
        "cpu_model",
    ):
        _mapping_string(host, key, "host")
    for key in ("logical_cpu_count", "physical_memory_bytes"):
        value = host.get(key)
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise BenchmarkProvenanceError(f"host.{key} must be a positive integer")

    _validate_metadata_commands(
        metadata.get("toolchains"), "toolchains", REQUIRED_TOOLS, versioned=True
    )
    _validate_metadata_commands(
        metadata.get("builds"), "builds", REQUIRED_BUILDS, versioned=False
    )
    _validate_metadata_files(
        metadata.get("dependency_locks"),
        "dependency_locks",
        set(REQUIRED_LOCKS),
    )
    _validate_metadata_files(metadata.get("artifacts"), "artifacts", REQUIRED_ARTIFACTS)

    fixtures = metadata.get("fixtures")
    if not isinstance(fixtures, list) or not fixtures:
        raise BenchmarkProvenanceError("run metadata.fixtures must be non-empty")
    fixture_ids: set[str] = set()
    for item in fixtures:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError("fixtures entries must be objects")
        fixture_id = _mapping_string(item, "id", "fixture")
        if fixture_id in fixture_ids:
            raise BenchmarkProvenanceError(f"duplicate fixture id: {fixture_id}")
        fixture_ids.add(fixture_id)
        _validate_sha256(item.get("sha256"), f"fixture {fixture_id}.sha256")

    invocation = _required_mapping(metadata, "invocation", "run metadata")
    if _mapping_string(invocation, "working_directory", "invocation") != ".":
        raise BenchmarkProvenanceError("invocation.working_directory must be .")
    _metadata_command_argv(invocation, "invocation")
    if invocation.get("repetitions") != repetitions:
        raise BenchmarkProvenanceError(f"invocation.repetitions must be {repetitions}")
    if (
        _mapping_string(invocation, "execution_order", "invocation")
        != "round-robin-by-repetition"
    ):
        raise BenchmarkProvenanceError("invocation.execution_order is invalid")


def capture_run_metadata(
    repo_root: Path,
    plan: ProvenancePlan,
    reference_python: Path,
    candidate_python: Path,
    invocation_argv: Sequence[str],
) -> dict[str, object]:
    """Capture the exact clean source, host, tools, locks, and built artifacts."""

    revision = _command_output(["git", "rev-parse", "HEAD"], repo_root)
    if not SHA_PATTERN.fullmatch(revision):
        raise BenchmarkProvenanceError("git did not return a full source revision")
    status = _command_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], repo_root
    )
    if plan.working_tree_policy == "require-clean" and status:
        raise BenchmarkProvenanceError(
            "benchmark source working tree is not clean; commit exact inputs first"
        )

    replacements = {
        "{harness_python}": sys.executable,
        "{reference_python}": str(reference_python.resolve()),
        "{candidate_python}": str(candidate_python.resolve()),
    }
    toolchains = []
    for tool in plan.tools:
        command = _resolve_command(tool.command_argv, replacements)
        toolchains.append(
            {
                "id": tool.id,
                "command_argv": _recorded_argv(command, repo_root),
                "version": _command_output(command, repo_root, combine_stderr=True),
            }
        )

    builds = [
        {
            "id": build.id,
            "command_argv": _recorded_argv(
                _resolve_command(build.command_argv, replacements), repo_root
            ),
            "flags": list(build.flags),
        }
        for build in plan.builds
    ]
    dependency_locks = [
        {
            "role": lock.role,
            "path": lock.path,
            "sha256": _sha256_file(repo_root / lock.path),
        }
        for lock in plan.dependency_locks
    ]

    rust_binary = repo_root / plan.scenario_suite.competitor_suite.rust_binary
    native_path = Path(
        _command_output(
            [
                str(candidate_python),
                "-c",
                "import pathlib, pdfplumber._native as n; print(pathlib.Path(n.__file__).resolve())",
            ],
            repo_root,
        )
    )
    artifacts = [
        _artifact_record(repo_root, "rust-benchmark-adapter", rust_binary),
        _artifact_record(repo_root, "candidate-python-native-extension", native_path),
    ]

    fixture_ids = sorted(
        {
            fixture_id
            for scenario in plan.scenario_suite.scenarios
            for fixture_id in scenario.fixture_ids
        }
    )
    fixtures = []
    for fixture_id in fixture_ids:
        fixture = plan.scenario_suite.competitor_suite.corpus.fixture(fixture_id)
        actual_sha256 = _sha256_file(repo_root / fixture.path)
        if actual_sha256 != fixture.sha256:
            raise BenchmarkProvenanceError(
                f"fixture {fixture_id} digest changed before execution"
            )
        fixtures.append({"id": fixture.id, "sha256": fixture.sha256})

    metadata: dict[str, object] = {
        "recorded_at_utc": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "source": {
            "repository": REPOSITORY,
            "revision": revision,
            "working_tree_clean": not status,
        },
        "host": _host_metadata(),
        "toolchains": toolchains,
        "builds": builds,
        "dependency_locks": dependency_locks,
        "artifacts": artifacts,
        "fixtures": fixtures,
        "invocation": {
            "working_directory": ".",
            "command_argv": _recorded_argv(invocation_argv, repo_root),
            "repetitions": plan.repetitions,
            "execution_order": plan.execution_order,
        },
    }
    validate_run_metadata(metadata, repetitions=plan.repetitions)
    return metadata


def write_local_run(
    output_path: Path,
    *,
    run_metadata: Mapping[str, object],
    records: Sequence[Mapping[str, object]],
    preflight_decisions: Sequence[Mapping[str, object]],
    scenario_timings: Sequence[Mapping[str, object]],
    statistical_summaries: Sequence[Mapping[str, object]],
) -> None:
    """Write a complete but explicitly unpublished SCORE-007 run."""

    invocation = _required_mapping(run_metadata, "invocation", "run metadata")
    repetitions = invocation.get("repetitions")
    if isinstance(repetitions, bool) or not isinstance(repetitions, int):
        raise BenchmarkProvenanceError("invocation.repetitions must be an integer")
    validate_run_metadata(run_metadata, repetitions=repetitions)
    expected_summaries = summarize_samples(scenario_timings, repetitions=repetitions)
    if _canonical_json(expected_summaries) != _canonical_json(statistical_summaries):
        raise BenchmarkProvenanceError(
            "statistical summaries do not match the retained raw samples"
        )
    payload = {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "run_metadata": dict(run_metadata),
        "records": list(records),
        "preflight_decisions": list(preflight_decisions),
        "scenario_timings": list(scenario_timings),
        "statistical_summaries": list(statistical_summaries),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def render_markdown(plan: ProvenancePlan) -> str:
    """Render the complete run-reproduction contract without publishing results."""

    lock_rows = [
        f"| `{lock.role}` | `{lock.path}` | SHA-256 recorded at run time |"
        for lock in plan.dependency_locks
    ]
    build_rows = [
        "| "
        + " | ".join(
            (
                f"`{build.id}`",
                "`" + " ".join(build.command_argv) + "`",
                ", ".join(f"`{flag}`" for flag in build.flags),
            )
        )
        + " |"
        for build in plan.builds
    ]
    return "\n".join(
        (
            "# Benchmark Run Provenance v0.3.0",
            "",
            f"Suite `{plan.id}` binds `{plan.scenario_suite_id}` to complete run metadata, {plan.repetitions} raw repetitions per eligible implementation/case, and deterministic statistical summaries.",
            "",
            "## Run identity",
            "",
            "A run is accepted only from a clean Git worktree. It records the exact repository revision, UTC capture time, operating-system name and release, architecture, Central Processing Unit model and logical count, physical memory, and the complete fixture identifier/SHA-256 set. The built Rust adapter and installed candidate native extension are also size- and digest-bound.",
            "",
            "Compiler and interpreter evidence includes the harness, reference, and candidate Python versions plus verbose Rust compiler, Cargo, and Maturin versions. Commands are retained as argument arrays from repository root rather than shell strings.",
            "",
            "## Build and dependency inputs",
            "",
            "| Artifact | Command | Material flags |",
            "|---|---|---|",
            *build_rows,
            "",
            "| Dependency role | Lock | Run record |",
            "|---|---|---|",
            *lock_rows,
            "",
            "The pinned Python reference environment is rebuilt from `compat/requirements-golden.txt` with hashes required. The candidate setup enforces Maturin 1.14.1, builds its local wheel in the release profile, and installs it with `--no-deps`; the Rust adapter uses its committed lock with release mode and the candidate's `parallel` feature.",
            "",
            "## Repetitions and statistics",
            "",
            f"Each exact-output-eligible key runs {plan.repetitions} times in `{plan.execution_order}` order. Every raw sample retains its repetition index, exact adapter argv, semantic-output digest, scenario state, and fixture digest. A summary is emitted only when repetitions 1 through {plan.repetitions} are all present and every non-time field remains identical.",
            "",
            "Summaries report sample size, minimum, median, arithmetic mean, maximum, sample standard deviation, and relative standard deviation for monotonic wall time. The ordered raw-sample array is SHA-256-bound into each summary. These descriptive statistics estimate observed run noise; they are not a confidence interval, regression threshold, or winner declaration.",
            "",
            "```console",
            "python3 scripts/run_benchmark_provenance.py --check",
            "python3 scripts/run_benchmark_provenance.py --build",
            "python3 scripts/run_benchmark_provenance.py --run --output /tmp/pdfplumber-rs-provenance.json",
            "```",
            "",
            "SCORE-007 results remain local and unpublished. Release-asset retention and result-removal policy remain open under SCORE-008 and SCORE-009, so no cross-project performance result is claimed.",
            "",
        )
    )


def _validate_dependency_locks(
    value: object, repo_root: Path
) -> tuple[DependencyLock, ...]:
    if not isinstance(value, list):
        raise BenchmarkProvenanceError("dependency_locks must be an array")
    locks: list[DependencyLock] = []
    for item in value:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError("dependency lock must be one table")
        role = _mapping_string(item, "role", "dependency lock")
        path = _repository_path(
            _mapping_string(item, "path", f"dependency lock {role}")
        )
        if not (repo_root / path).is_file():
            raise BenchmarkProvenanceError(f"dependency lock is missing: {path}")
        locks.append(DependencyLock(role=role, path=path))
    by_role = {lock.role: lock.path for lock in locks}
    if by_role != REQUIRED_LOCKS or len(locks) != len(REQUIRED_LOCKS):
        raise BenchmarkProvenanceError(
            f"dependency locks must be exactly {REQUIRED_LOCKS}"
        )
    return tuple(sorted(locks, key=lambda lock: lock.role))


def _validate_builds(value: object) -> tuple[BuildDefinition, ...]:
    if not isinstance(value, list):
        raise BenchmarkProvenanceError("builds must be an array")
    builds: list[BuildDefinition] = []
    for item in value:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError("build must be one table")
        build_id = _mapping_string(item, "id", "build")
        builds.append(
            BuildDefinition(
                id=build_id,
                command_argv=_required_string_array(
                    item, "command_argv", f"build {build_id}"
                ),
                flags=_required_string_array(item, "flags", f"build {build_id}"),
            )
        )
    if {build.id for build in builds} != REQUIRED_BUILDS or len(builds) != len(
        REQUIRED_BUILDS
    ):
        raise BenchmarkProvenanceError(
            f"build ids must be exactly {sorted(REQUIRED_BUILDS)}"
        )
    rust = next(build for build in builds if build.id == "rust-benchmark-adapter")
    for required in ("--release", "--locked", "features=parallel"):
        if required not in rust.flags:
            raise BenchmarkProvenanceError(f"Rust build lacks {required}")
    return tuple(sorted(builds, key=lambda build: build.id))


def _validate_tools(value: object) -> tuple[ToolDefinition, ...]:
    if not isinstance(value, list):
        raise BenchmarkProvenanceError("tools must be an array")
    tools: list[ToolDefinition] = []
    for item in value:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError("tool must be one table")
        tool_id = _mapping_string(item, "id", "tool")
        tools.append(
            ToolDefinition(
                id=tool_id,
                command_argv=_required_string_array(
                    item, "command_argv", f"tool {tool_id}"
                ),
            )
        )
    if {tool.id for tool in tools} != REQUIRED_TOOLS or len(tools) != len(
        REQUIRED_TOOLS
    ):
        raise BenchmarkProvenanceError(
            f"tool ids must be exactly {sorted(REQUIRED_TOOLS)}"
        )
    return tuple(sorted(tools, key=lambda tool: tool.id))


def _validate_metadata_commands(
    value: object,
    context: str,
    required_ids: set[str],
    *,
    versioned: bool,
) -> None:
    if not isinstance(value, list):
        raise BenchmarkProvenanceError(f"run metadata.{context} must be an array")
    actual_ids: list[str] = []
    for item in value:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError(f"{context} entries must be objects")
        item_id = _mapping_string(item, "id", context)
        actual_ids.append(item_id)
        _metadata_command_argv(item, f"{context} {item_id}")
        if versioned:
            _mapping_string(item, "version", f"{context} {item_id}")
        else:
            flags = item.get("flags")
            if (
                not isinstance(flags, list)
                or not flags
                or any(not isinstance(flag, str) or not flag for flag in flags)
            ):
                raise BenchmarkProvenanceError(
                    f"{context} {item_id}.flags must be a non-empty string array"
                )
    if set(actual_ids) != required_ids or len(actual_ids) != len(required_ids):
        raise BenchmarkProvenanceError(
            f"run metadata.{context} ids must be {sorted(required_ids)}"
        )
    if context == "builds":
        candidate = next(
            item
            for item in value
            if isinstance(item, dict) and item.get("id") == "candidate-python-wheel"
        )
        if "profile=release" not in candidate.get("flags", []):
            raise BenchmarkProvenanceError(
                "candidate-python-wheel must record profile=release"
            )


def _validate_metadata_files(
    value: object, context: str, required_roles: set[str]
) -> None:
    if not isinstance(value, list):
        raise BenchmarkProvenanceError(f"run metadata.{context} must be an array")
    actual_roles: list[str] = []
    for item in value:
        if not isinstance(item, dict):
            raise BenchmarkProvenanceError(f"{context} entries must be objects")
        role = _mapping_string(item, "role", context)
        actual_roles.append(role)
        _repository_path(_mapping_string(item, "path", f"{context} {role}"))
        _validate_sha256(item.get("sha256"), f"{context} {role}.sha256")
        if context == "artifacts":
            size = item.get("size_bytes")
            if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
                raise BenchmarkProvenanceError(
                    f"artifacts {role}.size_bytes must be positive"
                )
    if set(actual_roles) != required_roles or len(actual_roles) != len(required_roles):
        raise BenchmarkProvenanceError(
            f"run metadata.{context} roles must be {sorted(required_roles)}"
        )


def _sample_stable_projection(sample: Mapping[str, object]) -> dict[str, object]:
    stable = dict(sample)
    stable.pop("wall_time_ns", None)
    stable.pop("repetition", None)
    required = (
        "case_id",
        "scenario_id",
        "implementation",
        "fixtures",
        "measurement_scope",
        "clock",
        "timed_operation",
        "semantic_output_sha256",
        "command_argv",
    )
    for key in required:
        if key not in stable:
            raise BenchmarkProvenanceError(f"sample lacks {key}")
    return stable


def _sample_repetition(sample: Mapping[str, object]) -> int:
    value = sample.get("repetition")
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise BenchmarkProvenanceError("sample repetitions must be positive integers")
    return value


def _sample_wall_time(sample: Mapping[str, object]) -> int:
    value = sample.get("wall_time_ns")
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise BenchmarkProvenanceError("sample wall_time_ns must be positive")
    return value


def _host_metadata() -> dict[str, object]:
    system = platform.system()
    if system not in {"Darwin", "Linux"}:
        raise BenchmarkProvenanceError(f"unsupported benchmark host: {system}")
    cpu_model = ""
    if system == "Darwin":
        for name in ("machdep.cpu.brand_string", "hw.model"):
            completed = subprocess.run(
                ["sysctl", "-n", name],
                check=False,
                capture_output=True,
                text=True,
            )
            if completed.returncode == 0 and completed.stdout.strip():
                cpu_model = completed.stdout.strip()
                break
        memory_text = _command_output(["sysctl", "-n", "hw.memsize"], Path.cwd())
        physical_memory_bytes = int(memory_text)
    else:
        cpuinfo = Path("/proc/cpuinfo").read_text(encoding="utf-8")
        cpu_model = next(
            (
                line.split(":", 1)[1].strip()
                for line in cpuinfo.splitlines()
                if line.lower().startswith("model name") and ":" in line
            ),
            platform.processor(),
        )
        physical_memory_bytes = int(os.sysconf("SC_PAGE_SIZE")) * int(
            os.sysconf("SC_PHYS_PAGES")
        )
    logical_cpu_count = os.cpu_count()
    if not cpu_model or logical_cpu_count is None or physical_memory_bytes <= 0:
        raise BenchmarkProvenanceError("host hardware metadata is incomplete")
    return {
        "operating_system": system,
        "operating_system_release": platform.release(),
        "architecture": platform.machine(),
        "cpu_model": cpu_model,
        "logical_cpu_count": logical_cpu_count,
        "physical_memory_bytes": physical_memory_bytes,
    }


def _artifact_record(repo_root: Path, role: str, path: Path) -> dict[str, object]:
    resolved = path.resolve()
    if not resolved.is_file():
        raise BenchmarkProvenanceError(f"built artifact is missing: {path}")
    try:
        relative = resolved.relative_to(repo_root.resolve()).as_posix()
    except ValueError as error:
        raise BenchmarkProvenanceError(
            f"built artifact is outside the repository: {resolved}"
        ) from error
    return {
        "role": role,
        "path": relative,
        "sha256": _sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _resolve_command(
    command: Sequence[str], replacements: Mapping[str, str]
) -> list[str]:
    return [replacements.get(argument, argument) for argument in command]


def _recorded_argv(command: Sequence[str], repo_root: Path) -> list[str]:
    recorded = []
    root = repo_root.resolve()
    for argument in command:
        path = Path(argument)
        if path.is_absolute():
            try:
                argument = path.resolve().relative_to(root).as_posix()
            except ValueError:
                pass
        recorded.append(argument)
    return recorded


def _command_output(
    command: Sequence[str],
    cwd: Path,
    *,
    combine_stderr: bool = False,
) -> str:
    completed = subprocess.run(
        list(command),
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    output = completed.stdout
    if combine_stderr and completed.stderr:
        output += completed.stderr
    return output.strip()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


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
        raise BenchmarkProvenanceError("provenance data is not finite JSON") from error


def _mapping_string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value.strip():
        raise BenchmarkProvenanceError(f"{context}.{key} must be a non-empty string")
    return value


def _required_mapping(
    source: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise BenchmarkProvenanceError(f"{context}.{key} must be an object")
    return value


def _required_string(source: Mapping[str, object], key: str, context: str) -> str:
    return _mapping_string(source, key, context)


def _required_string_array(
    source: Mapping[str, object], key: str, context: str
) -> tuple[str, ...]:
    value = source.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(
            not isinstance(item, str) or not item or "\n" in item or "\x00" in item
            for item in value
        )
    ):
        raise BenchmarkProvenanceError(
            f"{context}.{key} must be a non-empty safe string array"
        )
    return tuple(value)


def _metadata_command_argv(
    source: Mapping[str, object], context: str
) -> tuple[str, ...]:
    return _required_string_array(source, "command_argv", context)


def _repository_path(value: str) -> str:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or value in {"", "."}:
        raise BenchmarkProvenanceError(f"path must be repository-relative: {value}")
    return path.as_posix()


def _validate_sha256(value: object, context: str) -> None:
    if not isinstance(value, str) or not SHA256_PATTERN.fullmatch(value):
        raise BenchmarkProvenanceError(f"{context} must be a SHA-256 digest")
