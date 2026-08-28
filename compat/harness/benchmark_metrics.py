"""Fail-closed resource and artifact metric contracts for SCORE-005."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_stages


class BenchmarkMetricError(ValueError):
    """A metric plan, resource envelope, or artifact sample is invalid."""


@dataclass(frozen=True)
class AllocatorDefinition:
    """One runtime-specific allocation observation method."""

    runtime: str
    method: str
    scope: str
    metrics: tuple[str, ...]


@dataclass(frozen=True)
class ArtifactDefinition:
    """One candidate-attributable build artifact."""

    id: str
    kind: str
    build_command: tuple[str, ...]
    paths: tuple[str, ...]


@dataclass(frozen=True)
class WasmStartupDefinition:
    """Fresh-process WebAssembly module-startup measurement."""

    adapter: str
    entry_path: str
    process_model: str
    clock: str
    clock_scope: str
    includes_process_launch: bool


@dataclass(frozen=True)
class MetricSuite:
    """Validated SCORE-005 measurement plan."""

    id: str
    release: str
    stage_suite_id: str
    resource_platforms: tuple[str, ...]
    wall_measurement_pass: str
    resource_measurement_pass: str
    cpu_clock: str
    cpu_scope: str
    peak_rss_scope: str
    allocators: tuple[AllocatorDefinition, ...]
    artifacts: tuple[ArtifactDefinition, ...]
    wasm_startup: WasmStartupDefinition | None

    def allocator(self, runtime: str) -> AllocatorDefinition | None:
        """Return the allocation method for one runtime."""

        for allocator in self.allocators:
            if allocator.runtime == runtime:
                return allocator
        return None


def audit_repository(
    repo_root: Path,
    metrics_path: Path,
    stage_suite: benchmark_stages.StageSuite,
) -> MetricSuite:
    """Load and validate the metric plan against the stage suite."""

    try:
        with metrics_path.open("rb") as metrics_file:
            source = tomllib.load(metrics_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkMetricError(
            f"cannot read metric plan: {metrics_path}"
        ) from error
    return validate_suite(source, repo_root, stage_suite)


def validate_suite(
    source: Mapping[str, object],
    repo_root: Path,
    stage_suite: benchmark_stages.StageSuite,
) -> MetricSuite:
    """Validate one parsed metric manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkMetricError("schema.version must be 1")
    raw_suite = _mapping(source, "suite", "manifest")
    suite_id = _string(raw_suite, "id", "suite")
    release = _string(raw_suite, "release", "suite")
    stage_suite_id = _string(raw_suite, "stage_suite_id", "suite")
    if release != stage_suite.release:
        raise BenchmarkMetricError("metric release differs from stage suite")
    if stage_suite_id != stage_suite.id:
        raise BenchmarkMetricError("metric plan names the wrong stage suite")

    passes = _mapping(source, "measurement_passes", "manifest")
    wall_measurement_pass = _string(passes, "wall", "measurement_passes")
    resource_measurement_pass = _string(passes, "resources", "measurement_passes")
    if wall_measurement_pass != "un-instrumented":
        raise BenchmarkMetricError("wall measurement pass must be un-instrumented")
    if resource_measurement_pass != "separate-instrumented":
        raise BenchmarkMetricError(
            "resource measurement pass must be separate-instrumented"
        )

    host = _mapping(source, "host", "manifest")
    resource_platforms = _string_array(host, "resource_platforms", "host")
    if resource_platforms != ("linux", "macos"):
        raise BenchmarkMetricError("resource hosts must be exactly Linux and macOS")

    cpu = _mapping(source, "cpu", "manifest")
    cpu_clock = _string(cpu, "clock", "cpu")
    cpu_scope = _string(cpu, "scope", "cpu")
    if cpu_clock != "process-cpu" or cpu_scope != "in-adapter-stage-only":
        raise BenchmarkMetricError("CPU metric must be stage-only process CPU")
    peak_rss = _mapping(source, "peak_resident_memory", "manifest")
    peak_rss_scope = _string(peak_rss, "scope", "peak_resident_memory")
    if peak_rss_scope != "adapter-process-lifetime-high-water":
        raise BenchmarkMetricError("peak resident memory must retain high-water scope")

    raw_allocators = source.get("allocators")
    if not isinstance(raw_allocators, list) or not raw_allocators:
        raise BenchmarkMetricError("allocators must be a non-empty array")
    allocators = tuple(_validate_allocator(value) for value in raw_allocators)
    if {allocator.runtime for allocator in allocators} != {"python", "rust"}:
        raise BenchmarkMetricError("allocators must define exactly Python and Rust")
    if len({allocator.runtime for allocator in allocators}) != len(allocators):
        raise BenchmarkMetricError("allocator runtimes must be unique")

    raw_artifacts = source.get("artifacts")
    if not isinstance(raw_artifacts, list) or not raw_artifacts:
        raise BenchmarkMetricError("artifacts must be a non-empty array")
    artifacts = tuple(_validate_artifact(value, repo_root) for value in raw_artifacts)
    if {artifact.id for artifact in artifacts} != {"native-cli", "wasm-node-package"}:
        raise BenchmarkMetricError("artifacts must be attributable native CLI and WASM")

    raw_startup = _mapping(source, "wasm_startup", "manifest")
    adapter = _repository_file(raw_startup, "adapter", "wasm_startup", repo_root)
    entry_path = _repository_path(raw_startup, "entry_path", "wasm_startup")
    process_model = _string(raw_startup, "process_model", "wasm_startup")
    clock = _string(raw_startup, "clock", "wasm_startup")
    clock_scope = _string(raw_startup, "clock_scope", "wasm_startup")
    includes_process_launch = raw_startup.get("includes_process_launch")
    if includes_process_launch is not False:
        raise BenchmarkMetricError("WASM startup must exclude process launch")
    if process_model != "fresh-process-per-sample":
        raise BenchmarkMetricError("WASM startup must use a fresh process")
    if clock != "monotonic-wall" or clock_scope != "node-module-load-only":
        raise BenchmarkMetricError(
            "WASM startup clock must cover Node module load only"
        )
    wasm_artifact = next(
        artifact for artifact in artifacts if artifact.id == "wasm-node-package"
    )
    if entry_path not in wasm_artifact.paths:
        raise BenchmarkMetricError(
            "WASM startup entry must be a measured bundle component"
        )

    return MetricSuite(
        id=suite_id,
        release=release,
        stage_suite_id=stage_suite_id,
        resource_platforms=resource_platforms,
        wall_measurement_pass=wall_measurement_pass,
        resource_measurement_pass=resource_measurement_pass,
        cpu_clock=cpu_clock,
        cpu_scope=cpu_scope,
        peak_rss_scope=peak_rss_scope,
        allocators=allocators,
        artifacts=artifacts,
        wasm_startup=WasmStartupDefinition(
            adapter=adapter,
            entry_path=entry_path,
            process_model=process_model,
            clock=clock,
            clock_scope=clock_scope,
            includes_process_launch=includes_process_launch,
        ),
    )


def build_resource_sample(
    *,
    stage: benchmark_stages.StageDefinition,
    untimed_record: Mapping[str, object],
    measured_outcome: Mapping[str, object],
    resource: Mapping[str, object],
    command_argv: Sequence[str],
) -> dict[str, object]:
    """Bind a separate resource pass to unchanged canonical output."""

    if _canonical_json(untimed_record.get("outcome")) != _canonical_json(
        measured_outcome
    ):
        raise BenchmarkMetricError("resource output drifted from untimed stage output")
    if measured_outcome.get("status") != "success":
        raise BenchmarkMetricError("only successful stage output can retain resources")
    if resource.get("stage_id") != stage.id:
        raise BenchmarkMetricError("resource envelope names the wrong stage")

    cpu = _mapping(resource, "cpu", "resource envelope")
    if cpu.get("clock") != "process-cpu" or cpu.get("scope") != "in-adapter-stage-only":
        raise BenchmarkMetricError("resource CPU scope or clock differs")
    cpu_time_ns = _nonnegative_integer(cpu, "time_ns", "resource CPU")
    peak_rss = _mapping(resource, "peak_resident_memory", "resource envelope")
    if peak_rss.get("scope") != "adapter-process-lifetime-high-water":
        raise BenchmarkMetricError("resource peak RSS scope differs")
    peak_resident_memory_bytes = _positive_integer(
        peak_rss,
        "bytes",
        "resource peak RSS",
    )
    allocations = _validate_allocation_sample(
        _mapping(resource, "allocations", "resource envelope")
    )

    implementation = _mapping(untimed_record, "implementation", "untimed record")
    fixture = _mapping(untimed_record, "fixture", "untimed record")
    return {
        "schema_version": 1,
        "case_id": f"{fixture.get('id')}:{stage.id}",
        "stage_id": stage.id,
        "implementation": dict(implementation),
        "fixture": dict(fixture),
        "measurement_pass": "separate-instrumented",
        "cpu_clock": "process-cpu",
        "cpu_scope": "in-adapter-stage-only",
        "cpu_time_ns": cpu_time_ns,
        "peak_resident_memory_scope": "adapter-process-lifetime-high-water",
        "peak_resident_memory_bytes": peak_resident_memory_bytes,
        "allocations": allocations,
        "semantic_output_sha256": _digest(measured_outcome),
        "command_argv": list(command_argv),
    }


def measure_artifact_sizes(
    repo_root: Path,
    suite: MetricSuite,
) -> list[dict[str, object]]:
    """Measure only manifest-declared candidate artifacts."""

    samples: list[dict[str, object]] = []
    for artifact in suite.artifacts:
        components: list[dict[str, object]] = []
        total_bytes = 0
        for relative_path in artifact.paths:
            path = repo_root / relative_path
            if not path.is_file():
                raise BenchmarkMetricError(f"artifact is missing: {relative_path}")
            size = path.stat().st_size
            total_bytes += size
            components.append({"path": relative_path, "bytes": size})
        samples.append(
            {
                "schema_version": 1,
                "artifact_id": artifact.id,
                "kind": artifact.kind,
                "attribution": "pdfplumber-rs-candidate-only",
                "build_command_argv": list(artifact.build_command),
                "components": components,
                "bytes": total_bytes,
            }
        )
    return samples


def validate_wasm_startup(
    suite: MetricSuite,
    decoded: Mapping[str, object],
    command_argv: Sequence[str],
) -> dict[str, object]:
    """Validate one in-process Node module load duration."""

    startup = suite.wasm_startup
    if startup is None:
        raise BenchmarkMetricError("WASM startup plan is missing")
    if decoded.get("clock") != startup.clock:
        raise BenchmarkMetricError("WASM startup emitted the wrong clock")
    if decoded.get("clock_scope") != startup.clock_scope:
        raise BenchmarkMetricError("WASM startup emitted the wrong scope")
    if decoded.get("process_model") != startup.process_model:
        raise BenchmarkMetricError("WASM startup emitted the wrong process model")
    if decoded.get("includes_process_launch") is not False:
        raise BenchmarkMetricError("WASM startup included process launch")
    wall_time_ns = _positive_integer(decoded, "wall_time_ns", "WASM startup")
    return {
        "schema_version": 1,
        "artifact_id": "wasm-node-package",
        "clock": startup.clock,
        "clock_scope": startup.clock_scope,
        "process_model": startup.process_model,
        "includes_process_launch": False,
        "wall_time_ns": wall_time_ns,
        "command_argv": list(command_argv),
    }


def write_local_run(
    output_path: Path,
    *,
    records: Sequence[Mapping[str, object]],
    preflight_decisions: Sequence[Mapping[str, object]],
    stage_timings: Sequence[Mapping[str, object]],
    stage_resources: Sequence[Mapping[str, object]],
    artifact_sizes: Sequence[Mapping[str, object]],
    wasm_startup: Sequence[Mapping[str, object]],
) -> None:
    """Write an unpublished run without merging unlike metric scopes."""

    payload = {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "records": list(records),
        "preflight_decisions": list(preflight_decisions),
        "stage_timings": list(stage_timings),
        "stage_resources": list(stage_resources),
        "artifact_sizes": list(artifact_sizes),
        "wasm_startup": list(wasm_startup),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def render_markdown(suite: MetricSuite) -> str:
    """Render measurement semantics without publishing local results."""

    allocator_rows = [
        "| Runtime | Method | Scope | Reported fields |",
        "|---|---|---|---|",
    ]
    for allocator in suite.allocators:
        allocator_rows.append(
            "| "
            + " | ".join(
                (
                    f"`{allocator.runtime}`",
                    f"`{allocator.method}`",
                    f"`{allocator.scope}`",
                    ", ".join(f"`{metric}`" for metric in allocator.metrics),
                )
            )
            + " |"
        )
    artifact_rows = [
        "| Artifact | Kind | Measured paths |",
        "|---|---|---|",
    ]
    for artifact in suite.artifacts:
        artifact_rows.append(
            f"| `{artifact.id}` | `{artifact.kind}` | "
            + ", ".join(f"`{path}`" for path in artifact.paths)
            + " |"
        )
    return "\n".join(
        (
            f"# Benchmark resource and artifact metrics {suite.release}",
            "",
            (
                f"Suite `{suite.id}` extends stage suite `{suite.stage_suite_id}` with "
                "resource, binary-size, WebAssembly bundle-size, and startup contracts."
            ),
            "",
            "## Measurement passes",
            "",
            (
                "Wall time remains an un-instrumented in-adapter component pass. CPU and "
                "allocation observations run in a separate instrumented invocation and are "
                "retained only when canonical output still exactly matches the untimed "
                "preflight result."
            ),
            "",
            (
                "CPU time is process CPU consumed inside the stage boundary. Peak resident "
                "memory is the adapter process-lifetime high-water mark, so it includes "
                "interpreter/runtime startup and the declared setup operations and is not "
                "described as stage-local memory."
            ),
            "",
            (
                "The resource adapters currently support Linux and macOS hosts. Other hosts "
                "fail closed instead of guessing peak-resident-memory units or APIs."
            ),
            "",
            *allocator_rows,
            "",
            (
                "Python and Rust allocation fields are not equivalent: `tracemalloc` sees "
                "Python-traced retained blocks and peak traced bytes, while the Rust global "
                "allocator counts gross allocations and requested bytes. The report keeps "
                "the method and field names attached to every sample."
            ),
            "",
            "## Candidate artifact costs",
            "",
            *artifact_rows,
            "",
            (
                "The native executable and WebAssembly runtime files are candidate-owned "
                "outputs. The combined Rust competitor adapter is deliberately excluded "
                "because its size cannot be attributed to one implementation."
            ),
            "",
            (
                "Each WebAssembly startup sample launches a fresh Node.js process, then an "
                "in-process monotonic clock covers synchronous module load and WebAssembly "
                "instantiation. Node.js process launch is outside the clock."
            ),
            "",
            "```console",
            "python3 scripts/run_benchmark_metrics.py --check",
            "python3 scripts/run_benchmark_metrics.py --build",
            (
                "python3 scripts/run_benchmark_metrics.py --run "
                "--output /tmp/pdfplumber-rs-metrics.json"
            ),
            "```",
            "",
            (
                "SCORE-005 component results are not published independently. SCORE-006 and "
                "SCORE-007 add scenario separation, complete environment capture, five raw "
                "repetitions, and statistical summaries. SCORE-008 publishes only the complete "
                "exact-tag result bundle; SCORE-009 re-audits its semantics and withdraws the "
                "three result assets if exact reproduction or output equivalence fails."
            ),
            "",
        )
    )


def _validate_allocator(value: object) -> AllocatorDefinition:
    if not isinstance(value, dict):
        raise BenchmarkMetricError("each allocator must be one table")
    runtime = _string(value, "runtime", "allocator")
    method = _string(value, "method", f"allocator {runtime}")
    scope = _string(value, "scope", f"allocator {runtime}")
    metrics = _string_array(value, "metrics", f"allocator {runtime}")
    if scope != "in-adapter-stage-only":
        raise BenchmarkMetricError(f"allocator {runtime} must be stage-only")
    expected = {
        "python": (
            "python-tracemalloc",
            ("retained_allocation_count", "retained_bytes", "peak_traced_bytes"),
        ),
        "rust": (
            "rust-counting-global-allocator",
            ("gross_allocation_count", "gross_allocated_bytes"),
        ),
    }.get(runtime)
    if expected is None or (method, metrics) != expected:
        raise BenchmarkMetricError(f"allocator {runtime} fields differ from contract")
    return AllocatorDefinition(runtime, method, scope, metrics)


def _validate_artifact(value: object, repo_root: Path) -> ArtifactDefinition:
    if not isinstance(value, dict):
        raise BenchmarkMetricError("each artifact must be one table")
    artifact_id = _string(value, "id", "artifact")
    kind = _string(value, "kind", f"artifact {artifact_id}")
    build_command = _string_array(
        value,
        "build_command",
        f"artifact {artifact_id}",
    )
    paths = _string_array(value, "paths", f"artifact {artifact_id}")
    for relative_path in paths:
        _resolve_repository_path(repo_root, relative_path)
    if artifact_id == "native-cli" and (
        kind != "native-executable" or paths != ("target/release/pdfplumber",)
    ):
        raise BenchmarkMetricError("native CLI artifact is not attributable")
    if artifact_id == "wasm-node-package" and (
        kind != "wasm-package"
        or paths
        != (
            "crates/pdfplumber-wasm/pkg-benchmark/pdfplumber_wasm_bg.wasm",
            "crates/pdfplumber-wasm/pkg-benchmark/pdfplumber_wasm.js",
        )
    ):
        raise BenchmarkMetricError("WASM artifact must contain runtime module and glue")
    return ArtifactDefinition(artifact_id, kind, build_command, paths)


def _validate_allocation_sample(source: Mapping[str, object]) -> dict[str, object]:
    method = source.get("method")
    if source.get("scope") != "in-adapter-stage-only":
        raise BenchmarkMetricError("allocation sample scope differs")
    if method == "python-tracemalloc":
        expected_fields = (
            "retained_allocation_count",
            "retained_bytes",
            "peak_traced_bytes",
        )
    elif method == "rust-counting-global-allocator":
        expected_fields = ("gross_allocation_count", "gross_allocated_bytes")
    else:
        raise BenchmarkMetricError("allocation sample method is unknown")
    sample: dict[str, object] = {
        "method": method,
        "scope": "in-adapter-stage-only",
    }
    for field in expected_fields:
        sample[field] = _nonnegative_integer(source, field, "allocation sample")
    expected_keys = {"method", "scope", *expected_fields}
    if set(source) != expected_keys:
        raise BenchmarkMetricError("allocation sample fields differ from method")
    return sample


def _mapping(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> Mapping[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise BenchmarkMetricError(f"{context}.{key} must be one table")
    return value


def _string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value:
        raise BenchmarkMetricError(f"{context}.{key} must be a non-empty string")
    return value


def _string_array(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> tuple[str, ...]:
    value = source.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise BenchmarkMetricError(f"{context}.{key} must be a non-empty string array")
    result = tuple(value)
    if len(set(result)) != len(result):
        raise BenchmarkMetricError(f"{context}.{key} must not contain duplicates")
    return result


def _repository_path(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> str:
    return _string(source, key, context)


def _repository_file(
    source: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    relative_path = _string(source, key, context)
    path = _resolve_repository_path(repo_root, relative_path)
    if not path.is_file():
        raise BenchmarkMetricError(f"{context}.{key} is missing: {relative_path}")
    return relative_path


def _resolve_repository_path(repo_root: Path, relative_path: str) -> Path:
    path = Path(relative_path)
    if path.is_absolute() or ".." in path.parts:
        raise BenchmarkMetricError(
            f"path must stay repository-relative: {relative_path}"
        )
    resolved = (repo_root / path).resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError as error:
        raise BenchmarkMetricError(
            f"path escapes repository: {relative_path}"
        ) from error
    return resolved


def _nonnegative_integer(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> int:
    value = source.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise BenchmarkMetricError(f"{context}.{key} must be a non-negative integer")
    return value


def _positive_integer(
    source: Mapping[str, object],
    key: str,
    context: str,
) -> int:
    value = _nonnegative_integer(source, key, context)
    if value == 0:
        raise BenchmarkMetricError(f"{context}.{key} must be positive")
    return value


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
        raise BenchmarkMetricError("metric value is not finite JSON") from error


def _digest(value: object) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()
