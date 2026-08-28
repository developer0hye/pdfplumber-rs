#!/usr/bin/env python3
"""Validate, build, or execute the SCORE-005 metric suite."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
for import_path in (REPO_ROOT, REPO_ROOT / "scripts"):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

import run_stage_benchmarks as stage_runner

from compat.harness import benchmark_metrics, benchmark_stages

METRICS_PATH = REPO_ROOT / "benchmarks" / "metrics-v0.3.0.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "metrics-v0.3.0.md"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true")
    modes.add_argument("--write-report", action="store_true")
    modes.add_argument("--build", action="store_true")
    modes.add_argument("--run", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--reference-python",
        type=Path,
        default=stage_runner.DEFAULT_REFERENCE_PYTHON,
    )
    parser.add_argument(
        "--candidate-python",
        type=Path,
        default=stage_runner.DEFAULT_CANDIDATE_PYTHON,
    )
    return parser.parse_args()


def load_suites() -> tuple[benchmark_stages.StageSuite, benchmark_metrics.MetricSuite]:
    stage_suite = stage_runner.load_suite()
    metric_suite = benchmark_metrics.audit_repository(
        REPO_ROOT,
        METRICS_PATH,
        stage_suite,
    )
    return stage_suite, metric_suite


def check_suite(metric_suite: benchmark_metrics.MetricSuite) -> None:
    if not REPORT_PATH.is_file():
        raise benchmark_metrics.BenchmarkMetricError(
            f"metric report is missing: {REPORT_PATH}"
        )
    if REPORT_PATH.read_text(encoding="utf-8") != benchmark_metrics.render_markdown(
        metric_suite
    ):
        raise benchmark_metrics.BenchmarkMetricError(
            f"metric report is stale: {REPORT_PATH}"
        )
    adapter_sources = (
        REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber.py",
        REPO_ROOT / "benchmarks" / "adapters" / "python_pdfplumber_rs.py",
        REPO_ROOT / "benchmarks" / "adapters" / "rust" / "src" / "main.rs",
    )
    for adapter_source in adapter_sources:
        if "--resources" not in adapter_source.read_text(encoding="utf-8"):
            raise benchmark_metrics.BenchmarkMetricError(
                f"adapter lacks resource protocol: {adapter_source}"
            )


def build_artifacts(metric_suite: benchmark_metrics.MetricSuite) -> None:
    stage_runner.build_adapters()
    for artifact in metric_suite.artifacts:
        subprocess.run(
            list(artifact.build_command),
            cwd=REPO_ROOT,
            check=True,
        )


def resource_outcome(
    decoded: Mapping[str, object],
    stage_id: str,
) -> tuple[dict[str, object], Mapping[str, object]]:
    outcome = dict(decoded)
    if outcome.pop("timing", None) is not None:
        raise benchmark_metrics.BenchmarkMetricError(
            "resource invocation emitted a wall timing"
        )
    resource = outcome.pop("resources", None)
    if not isinstance(resource, dict) or resource.get("stage_id") != stage_id:
        raise benchmark_metrics.BenchmarkMetricError(
            f"adapter lacks an exact resource envelope for {stage_id}"
        )
    return outcome, resource


def execute_resource_pass(
    stage_suite: benchmark_stages.StageSuite,
    metric_suite: benchmark_metrics.MetricSuite,
    records: list[dict[str, object]],
    stage_timings: list[dict[str, object]],
    reference_python: Path,
    candidate_python: Path,
) -> list[dict[str, object]]:
    """Repeat only exact wall-eligible cases with resource instrumentation."""

    host_platform = "macos" if sys.platform == "darwin" else sys.platform
    if host_platform not in metric_suite.resource_platforms:
        raise benchmark_metrics.BenchmarkMetricError(
            f"resource metrics are unsupported on host platform: {sys.platform}"
        )

    records_by_key: dict[tuple[str, str, str], dict[str, object]] = {}
    for record in records:
        stage = record.get("stage")
        fixture = record.get("fixture")
        implementation = record.get("implementation")
        if not all(
            isinstance(value, dict) for value in (stage, fixture, implementation)
        ):
            raise benchmark_metrics.BenchmarkMetricError("stage record is malformed")
        assert isinstance(stage, dict)
        assert isinstance(fixture, dict)
        assert isinstance(implementation, dict)
        key = (
            str(stage.get("id")),
            str(fixture.get("id")),
            str(implementation.get("id")),
        )
        records_by_key[key] = record

    fixtures = {
        fixture.id: fixture for fixture in stage_suite.competitor_suite.corpus.fixtures
    }
    samples: list[dict[str, object]] = []
    for timing in stage_timings:
        stage_id = timing.get("stage_id")
        implementation = timing.get("implementation")
        fixture = timing.get("fixture")
        if (
            not isinstance(stage_id, str)
            or not isinstance(implementation, dict)
            or not isinstance(fixture, dict)
        ):
            raise benchmark_metrics.BenchmarkMetricError("stage timing is malformed")
        implementation_id = implementation.get("id")
        fixture_id = fixture.get("id")
        if not isinstance(implementation_id, str) or not isinstance(fixture_id, str):
            raise benchmark_metrics.BenchmarkMetricError(
                "stage timing identity is malformed"
            )
        stage = stage_suite.stage(stage_id)
        fixture_definition = fixtures.get(fixture_id)
        if stage is None or fixture_definition is None:
            raise benchmark_metrics.BenchmarkMetricError(
                "stage timing identity is unknown"
            )
        command = stage_runner.adapter_command(
            stage_suite,
            implementation_id,
            stage_id,
            fixture_definition.path,
            fixture_definition.password,
            reference_python,
            candidate_python,
            timed=False,
            resources=True,
        )
        measured_outcome, resource = resource_outcome(
            stage_runner.run_adapter(command),
            stage_id,
        )
        samples.append(
            benchmark_metrics.build_resource_sample(
                stage=stage,
                untimed_record=records_by_key[
                    (stage_id, fixture_id, implementation_id)
                ],
                measured_outcome=measured_outcome,
                resource=resource,
                command_argv=command,
            )
        )
    return samples


def execute_wasm_startup(
    metric_suite: benchmark_metrics.MetricSuite,
) -> list[dict[str, object]]:
    startup = metric_suite.wasm_startup
    if startup is None:
        raise benchmark_metrics.BenchmarkMetricError("WASM startup plan is missing")
    command = ["node", startup.adapter, startup.entry_path]
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    try:
        decoded = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise benchmark_metrics.BenchmarkMetricError(
            "WASM startup adapter emitted invalid JSON"
        ) from error
    if not isinstance(decoded, dict):
        raise benchmark_metrics.BenchmarkMetricError(
            "WASM startup outcome must be one object"
        )
    return [benchmark_metrics.validate_wasm_startup(metric_suite, decoded, command)]


def write_report(metric_suite: benchmark_metrics.MetricSuite) -> None:
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(
        benchmark_metrics.render_markdown(metric_suite),
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    if args.run != (args.output is not None):
        print("--output is required exactly with --run", file=sys.stderr)
        return 2
    try:
        stage_suite, metric_suite = load_suites()
        if args.write_report:
            write_report(metric_suite)
            print(f"Wrote {REPORT_PATH.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            check_suite(metric_suite)
            print("Metric suite OK: resource and artifact metrics are separated")
            return 0
        if args.build:
            build_artifacts(metric_suite)
            print("Built benchmark adapters and candidate artifacts")
            return 0

        assert args.output is not None
        records, decisions, stage_timings = stage_runner.execute_local_run(
            stage_suite,
            args.reference_python,
            args.candidate_python,
        )
        stage_resources = execute_resource_pass(
            stage_suite,
            metric_suite,
            records,
            stage_timings,
            args.reference_python,
            args.candidate_python,
        )
        artifact_sizes = benchmark_metrics.measure_artifact_sizes(
            REPO_ROOT,
            metric_suite,
        )
        wasm_startup = execute_wasm_startup(metric_suite)
        benchmark_metrics.write_local_run(
            args.output,
            records=records,
            preflight_decisions=decisions,
            stage_timings=stage_timings,
            stage_resources=stage_resources,
            artifact_sizes=artifact_sizes,
            wasm_startup=wasm_startup,
        )
        print(
            f"Wrote local unpublished metric run: {len(stage_timings)} wall and "
            f"{len(stage_resources)} resource samples"
        )
        return 0
    except (
        benchmark_metrics.BenchmarkMetricError,
        benchmark_stages.BenchmarkStageError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Benchmark metric error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
