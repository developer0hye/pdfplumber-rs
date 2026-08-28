#!/usr/bin/env python3
"""Validate, build, or execute the local SCORE-007 benchmark run."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_provenance, benchmark_scenarios
from scripts import run_benchmark_scenarios as scenario_runner

PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "provenance-v0.3.0.md"
DEFAULT_REFERENCE_PYTHON = REPO_ROOT / ".venv-reference" / "bin" / "python"
DEFAULT_CANDIDATE_PYTHON = REPO_ROOT / ".venv-candidate" / "bin" / "python"


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
        default=DEFAULT_REFERENCE_PYTHON,
    )
    parser.add_argument(
        "--candidate-python",
        type=Path,
        default=DEFAULT_CANDIDATE_PYTHON,
    )
    return parser.parse_args()


def load_plan() -> benchmark_provenance.ProvenancePlan:
    return benchmark_provenance.audit_repository(
        REPO_ROOT,
        PROVENANCE_PATH,
        scenario_runner.SCENARIOS_PATH,
        scenario_runner.SUITE_PATH,
        scenario_runner.CORPUS_PATH,
        scenario_runner.POLICY_PATH,
        scenario_runner.REGISTRY_PATH,
    )


def check_plan(plan: benchmark_provenance.ProvenancePlan) -> None:
    scenario_runner.check_suite(plan.scenario_suite)
    if not REPORT_PATH.is_file():
        raise benchmark_provenance.BenchmarkProvenanceError(
            f"provenance report is missing: {REPORT_PATH}"
        )
    if REPORT_PATH.read_text(encoding="utf-8") != benchmark_provenance.render_markdown(
        plan
    ):
        raise benchmark_provenance.BenchmarkProvenanceError(
            f"provenance report is stale: {REPORT_PATH}"
        )
    workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    if "python scripts/run_benchmark_provenance.py --check" not in workflow:
        raise benchmark_provenance.BenchmarkProvenanceError(
            "Continuous Integration lacks the provenance drift gate"
        )
    public_links = {
        REPO_ROOT / "README.md": "docs/benchmarks/provenance-v0.3.0.md",
        REPO_ROOT / "docs" / "comparison.md": "benchmarks/provenance-v0.3.0.md",
    }
    for path, link in public_links.items():
        if link not in path.read_text(encoding="utf-8"):
            raise benchmark_provenance.BenchmarkProvenanceError(
                f"public benchmark link is missing from {path.relative_to(REPO_ROOT)}"
            )


def build_environment(plan: benchmark_provenance.ProvenancePlan) -> None:
    reference_environment = os.environ.copy()
    reference_environment["PDFPLUMBER_RS_REQUIRE_PINNED_PYTHON"] = "1"
    reference_environment["PDFPLUMBER_RS_REFERENCE_PYTHON"] = "python3.13"
    subprocess.run(
        ["bash", "scripts/setup_golden_venv.sh"],
        cwd=REPO_ROOT,
        env=reference_environment,
        check=True,
    )
    candidate_build = next(
        build for build in plan.builds if build.id == "candidate-python-wheel"
    )
    subprocess.run(
        list(candidate_build.command_argv),
        cwd=REPO_ROOT,
        check=True,
    )
    scenario_runner.build_adapters()


def write_report(plan: benchmark_provenance.ProvenancePlan) -> None:
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(
        benchmark_provenance.render_markdown(plan),
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    if args.run != (args.output is not None):
        print("--output is required exactly with --run", file=sys.stderr)
        return 2
    try:
        plan = load_plan()
        if args.write_report:
            write_report(plan)
            print(f"Wrote {REPORT_PATH.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            check_plan(plan)
            print(
                f"Provenance suite OK: {plan.repetitions} repetitions, "
                f"{len(plan.dependency_locks)} dependency locks"
            )
            return 0
        if args.build:
            build_environment(plan)
            print("Built pinned reference, candidate, and competitor environments")
            return 0

        assert args.output is not None
        run_metadata = benchmark_provenance.capture_run_metadata(
            REPO_ROOT,
            plan,
            args.reference_python,
            args.candidate_python,
            sys.argv,
        )
        records, decisions, scenario_timings = scenario_runner.execute_local_run(
            plan.scenario_suite,
            args.reference_python,
            args.candidate_python,
            repetitions=plan.repetitions,
        )
        statistical_summaries = benchmark_provenance.summarize_samples(
            scenario_timings,
            repetitions=plan.repetitions,
        )
        benchmark_provenance.write_local_run(
            args.output,
            run_metadata=run_metadata,
            records=records,
            preflight_decisions=decisions,
            scenario_timings=scenario_timings,
            statistical_summaries=statistical_summaries,
        )
        print(
            f"Wrote local unpublished provenance run: {len(records)} records, "
            f"{len(scenario_timings)} raw samples, "
            f"{len(statistical_summaries)} summaries"
        )
        return 0
    except (
        benchmark_provenance.BenchmarkProvenanceError,
        benchmark_scenarios.BenchmarkScenarioError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Benchmark provenance error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
