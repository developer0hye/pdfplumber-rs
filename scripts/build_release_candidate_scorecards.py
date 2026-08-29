#!/usr/bin/env python3
"""Validate or build retained release-candidate scorecard assets."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import (
    compatibility_scorecard,
    release_candidate_scorecards,
    workflow_scorecard,
)
from scripts import generate_workflow_scorecard

POLICY_PATH = REPO_ROOT / "scorecards" / "release-candidates-v0.3.0.toml"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-candidate-scorecards.yml"
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true")
    modes.add_argument("--build-assets", action="store_true")
    parser.add_argument("--candidate-id")
    parser.add_argument("--source-revision")
    parser.add_argument("--run-url")
    parser.add_argument("--benchmark-run", type=Path)
    parser.add_argument("--parity-report", type=Path)
    parser.add_argument("--source-artifact", type=Path)
    parser.add_argument("--wheel", type=Path)
    parser.add_argument("--output-dir", type=Path)
    return parser.parse_args()


def load_json(path: Path, context: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"cannot read {context}: {path}"
        ) from error
    if not isinstance(value, dict):
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"{context} root must be an object"
        )
    return value


def command_output(*arguments: str) -> str:
    try:
        completed = subprocess.run(
            arguments,
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"cannot record tool version: {' '.join(arguments)}"
        ) from error
    output = completed.stdout.strip() or completed.stderr.strip()
    if not output:
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"tool version command returned no output: {' '.join(arguments)}"
        )
    return output.splitlines()[0]


def file_sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"cannot read candidate artifact: {path}"
        ) from error


def platform_id() -> str:
    raw = (
        f"{platform.system()}-{platform.release()}-{platform.machine()}-"
        f"cpython-{sys.version_info.major}.{sys.version_info.minor}"
    ).lower()
    return re.sub(r"[^a-z0-9.-]+", "-", raw).strip("-")


def build_compatibility(
    policy: release_candidate_scorecards.HistoryPolicy,
    *,
    candidate_id: str,
    source_revision: str,
    source_artifact: Path,
    wheel: Path,
    parity_report: dict[str, object],
) -> tuple[dict[str, object], str]:
    if policy.corpus is None or policy.corpus_sha256 is None:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "repository policy has no compatibility corpus"
        )
    if policy.release_version is None:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "repository policy has no release version"
        )
    runner_platform = compatibility_scorecard.Platform(
        id=platform_id(),
        system=platform.system(),
        release=platform.release(),
        machine=platform.machine(),
        python_version=platform.python_version(),
    )
    toolchain = compatibility_scorecard.Toolchain(
        rustc=command_output("rustc", "--version"),
        cargo=command_output("cargo", "--version"),
        builder=command_output(sys.executable, "-m", "maturin", "--version"),
    )
    evidence = (
        "compat/upstream.toml",
        "compat/requirements-golden.txt",
        "compat/fixture-provenance.toml",
        "scripts/parity_report.py",
        "scripts/generate_option_matrix.py",
    )
    machine = compatibility_scorecard.build(
        subject_version=policy.release_version,
        subject_revision=source_revision,
        corpus=policy.corpus,
        corpus_sha256=policy.corpus_sha256,
        runs=(
            compatibility_scorecard.RunInput(
                id=f"{candidate_id}-source-api",
                platform=runner_platform,
                artifact_type="source",
                artifact_name=source_artifact.name,
                artifact_sha256=file_sha256(source_artifact),
                toolchain=toolchain,
                command=(
                    ".venv-reference/bin/python scripts/parity_report.py "
                    "--repo <candidate> --fixtures <candidate> "
                    "--candidate-options <candidate-options.json> "
                    "--json <parity-report.json>"
                ),
                report=parity_report,
                evidence=evidence,
                scopes=("api",),
            ),
            compatibility_scorecard.RunInput(
                id=f"{candidate_id}-wheel-option",
                platform=runner_platform,
                artifact_type="wheel",
                artifact_name=wheel.name,
                artifact_sha256=file_sha256(wheel),
                toolchain=toolchain,
                command=(
                    ".venv-candidate/bin/python scripts/generate_option_matrix.py "
                    "--candidate-output <candidate-options.json>"
                ),
                report=parity_report,
                evidence=evidence,
                scopes=("option",),
            ),
        ),
    )
    if policy.workflow_source_path is None:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "repository policy has no workflow definitions"
        )
    workflow_source = generate_workflow_scorecard.load_toml(policy.workflow_source_path)
    machine_bytes = compatibility_scorecard.render(machine).encode("utf-8")
    machine_name = release_candidate_scorecards.asset_names(policy, candidate_id)[
        "compatibility"
    ]
    workflow = workflow_scorecard.build(
        machine,
        generate_workflow_scorecard.workflow_definitions(workflow_source),
        machine_path=machine_name,
        machine_sha256=hashlib.sha256(machine_bytes).hexdigest(),
        indexed_fixture_ids=tuple(fixture.path for fixture in policy.corpus.fixtures),
    )
    return machine, workflow_scorecard.render(workflow)


def check_repository(policy: release_candidate_scorecards.HistoryPolicy) -> None:
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    required_workflow = (
        "workflow_call:",
        "workflow_dispatch:",
        f"runs-on: {policy.runner}",
        "python scripts/run_benchmark_provenance.py --build",
        "python scripts/run_benchmark_provenance.py --run",
        "scripts/setup_golden_venv.sh",
        "scripts/parity_report.py",
        "continue-on-error: true",
        "scripts/build_release_candidate_scorecards.py --build-assets",
        f"retention-days: {policy.artifact_retention_days}",
    )
    for fragment in required_workflow:
        if fragment not in workflow:
            raise release_candidate_scorecards.ScorecardHistoryError(
                f"candidate workflow lacks {fragment!r}"
            )
    for prohibited in (
        "contents: write",
        "issues: write",
        "pull-requests: write",
    ):
        if prohibited in workflow:
            raise release_candidate_scorecards.ScorecardHistoryError(
                f"candidate workflow requests prohibited permission {prohibited!r}"
            )

    release = RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
    for fragment in (
        "uses: ./.github/workflows/release-candidate-scorecards.yml",
        "needs: [ci, metadata, scorecards]",
        "pattern: release-candidate-scorecards-*",
        "release-scorecards/*",
    ):
        if fragment not in release:
            raise release_candidate_scorecards.ScorecardHistoryError(
                f"release workflow lacks {fragment!r}"
            )
    scorecard_gated_candidates = (
        "needs: [release-artifacts, metadata, scorecards, integrity]",
        "needs: [release-artifacts, scorecards, integrity]",
        "needs: [ci, metadata, scorecards]",
    )
    for dependency in scorecard_gated_candidates:
        if dependency not in release:
            raise release_candidate_scorecards.ScorecardHistoryError(
                "every independent registry candidate must wait for scorecards"
            )
    ci = CI_PATH.read_text(encoding="utf-8")
    if "python scripts/build_release_candidate_scorecards.py --check" not in ci:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "Continuous Integration lacks the candidate-history drift gate"
        )
    public_links = {
        REPO_ROOT / "README.md": "docs/scorecards/release-candidate-history-v0.3.md",
        REPO_ROOT / "ROADMAP.md": "docs/scorecards/release-candidate-history-v0.3.md",
        REPO_ROOT
        / "docs"
        / "comparison.md": "scorecards/release-candidate-history-v0.3.md",
    }
    for path, link in public_links.items():
        if link not in path.read_text(encoding="utf-8"):
            raise release_candidate_scorecards.ScorecardHistoryError(
                f"public candidate-history link is missing from {path.relative_to(REPO_ROOT)}"
            )


def build_assets(
    policy: release_candidate_scorecards.HistoryPolicy,
    arguments: argparse.Namespace,
) -> release_candidate_scorecards.AssetBundle:
    required = {
        "candidate_id": arguments.candidate_id,
        "source_revision": arguments.source_revision,
        "run_url": arguments.run_url,
        "benchmark_run": arguments.benchmark_run,
        "parity_report": arguments.parity_report,
        "source_artifact": arguments.source_artifact,
        "wheel": arguments.wheel,
        "output_dir": arguments.output_dir,
    }
    missing = [name for name, value in required.items() if value is None]
    if missing:
        raise release_candidate_scorecards.ScorecardHistoryError(
            f"--build-assets is missing arguments: {', '.join(missing)}"
        )
    assert arguments.candidate_id is not None
    assert arguments.source_revision is not None
    assert arguments.run_url is not None
    assert arguments.benchmark_run is not None
    assert arguments.parity_report is not None
    assert arguments.source_artifact is not None
    assert arguments.wheel is not None
    assert arguments.output_dir is not None
    head = command_output("git", "rev-parse", "HEAD")
    if head != arguments.source_revision:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "explicit source revision does not match checked-out HEAD"
        )
    benchmark_run = load_json(arguments.benchmark_run, "benchmark run")
    parity_report = load_json(arguments.parity_report, "parity report")
    machine, workflow_report = build_compatibility(
        policy,
        candidate_id=arguments.candidate_id,
        source_revision=arguments.source_revision,
        source_artifact=arguments.source_artifact,
        wheel=arguments.wheel,
        parity_report=parity_report,
    )
    entry = release_candidate_scorecards.build_entry(
        policy,
        candidate_id=arguments.candidate_id,
        source_revision=arguments.source_revision,
        run_url=arguments.run_url,
        benchmark_run=benchmark_run,
        compatibility=machine,
        workflow_report=workflow_report,
    )
    if policy.history_path is None:
        raise release_candidate_scorecards.ScorecardHistoryError(
            "repository policy has no history path"
        )
    history = release_candidate_scorecards.append_entry(
        policy,
        release_candidate_scorecards.load_history(policy.history_path),
        entry,
    )
    return release_candidate_scorecards.write_assets(
        policy,
        arguments.output_dir,
        entry=history["runs"][-1],
        history=history,
        benchmark_run=benchmark_run,
        compatibility=machine,
        workflow_report=workflow_report,
    )


def main() -> int:
    arguments = parse_args()
    try:
        policy = release_candidate_scorecards.audit_repository(REPO_ROOT, POLICY_PATH)
        check_repository(policy)
        if arguments.check:
            build_only = (
                arguments.candidate_id,
                arguments.source_revision,
                arguments.run_url,
                arguments.benchmark_run,
                arguments.parity_report,
                arguments.source_artifact,
                arguments.wheel,
                arguments.output_dir,
            )
            if any(value is not None for value in build_only):
                raise release_candidate_scorecards.ScorecardHistoryError(
                    "asset arguments are accepted only with --build-assets"
                )
            history = release_candidate_scorecards.load_history(policy.history_path)
            print(
                "Release-candidate scorecard history OK: "
                f"{len(history['runs'])} retained run(s), {policy.runner}"
            )
            return 0
        bundle = build_assets(policy, arguments)
        print("Wrote release-candidate scorecards: " + ", ".join(bundle.names))
        return 0
    except (
        OSError,
        compatibility_scorecard.ScorecardError,
        release_candidate_scorecards.ScorecardHistoryError,
        workflow_scorecard.WorkflowScorecardError,
    ) as error:
        print(f"release-candidate scorecards failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
