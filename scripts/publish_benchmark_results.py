#!/usr/bin/env python3
"""Validate or build the versioned SCORE-008 benchmark release assets."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import benchmark_results, benchmark_retention

RETENTION_PATH = REPO_ROOT / "benchmarks" / "result-retention-v0.3.0.toml"
PUBLICATION_PATH = REPO_ROOT / "benchmarks" / "results-v0.3.0.toml"
PROVENANCE_PATH = REPO_ROOT / "benchmarks" / "provenance-v0.3.0.toml"
SCENARIOS_PATH = REPO_ROOT / "benchmarks" / "scenarios-v0.3.0.toml"
SUITE_PATH = REPO_ROOT / "benchmarks" / "competitors-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
POLICY_PATH = REPO_ROOT / "benchmarks" / "equivalence-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
INDEX_PATH = REPO_ROOT / "docs" / "benchmarks" / "results-v0.3.0.md"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "benchmark-results.yml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true")
    modes.add_argument("--write-index", action="store_true")
    modes.add_argument("--build-assets", action="store_true")
    parser.add_argument("--input", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--release-tag")
    parser.add_argument("--source-revision")
    return parser.parse_args()


def load_plan() -> benchmark_results.PublicationPlan:
    return benchmark_results.audit_repository(
        REPO_ROOT,
        PUBLICATION_PATH,
        PROVENANCE_PATH,
        SCENARIOS_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def load_retention_plan() -> benchmark_retention.RetentionPlan:
    return benchmark_retention.audit_repository(
        REPO_ROOT,
        RETENTION_PATH,
        PUBLICATION_PATH,
        PROVENANCE_PATH,
        SCENARIOS_PATH,
        SUITE_PATH,
        CORPUS_PATH,
        POLICY_PATH,
        REGISTRY_PATH,
    )


def check_plan(plan: benchmark_results.PublicationPlan) -> None:
    if not INDEX_PATH.is_file():
        raise benchmark_results.BenchmarkResultError(
            f"benchmark result index is missing: {INDEX_PATH}"
        )
    if INDEX_PATH.read_text(encoding="utf-8") != benchmark_retention.render_index(
        load_retention_plan()
    ):
        raise benchmark_results.BenchmarkResultError(
            f"benchmark result index is stale: {INDEX_PATH}"
        )
    if not WORKFLOW_PATH.is_file():
        raise benchmark_results.BenchmarkResultError(
            f"benchmark release workflow is missing: {WORKFLOW_PATH}"
        )
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    required_workflow_fragments = (
        "benchmark-results-v*",
        f"runs-on: {plan.runner}",
        "python-version: \"3.13.12\"",
        "toolchain: 1.98.0",
        "maturin==1.14.1",
        "python scripts/run_benchmark_provenance.py --build",
        "python scripts/run_benchmark_provenance.py --run",
        "python scripts/publish_benchmark_results.py --build-assets",
        "softprops/action-gh-release@v2",
        plan.raw_asset,
        plan.report_asset,
        plan.checksums_asset,
    )
    for fragment in required_workflow_fragments:
        if fragment not in workflow:
            raise benchmark_results.BenchmarkResultError(
                f"benchmark release workflow lacks {fragment!r}"
            )
    ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    if "python scripts/publish_benchmark_results.py --check" not in ci:
        raise benchmark_results.BenchmarkResultError(
            "Continuous Integration lacks the result-publication drift gate"
        )
    public_links = {
        REPO_ROOT / "README.md": "docs/benchmarks/results-v0.3.0.md",
        REPO_ROOT / "docs" / "comparison.md": "benchmarks/results-v0.3.0.md",
        REPO_ROOT / "crates" / "pdfplumber-py" / "README.md": (
            "../../docs/benchmarks/results-v0.3.0.md"
        ),
        REPO_ROOT / "crates" / "pdfplumber-wasm" / "README.md": (
            "../../docs/benchmarks/results-v0.3.0.md"
        ),
        REPO_ROOT / "crates" / "pdfplumber" / "benches" / "README.md": (
            "../../../docs/benchmarks/results-v0.3.0.md"
        ),
    }
    for path, link in public_links.items():
        if link not in path.read_text(encoding="utf-8"):
            raise benchmark_results.BenchmarkResultError(
                f"public benchmark result link is missing from {path.relative_to(REPO_ROOT)}"
            )


def git_output(*arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def exact_tag_revision(plan: benchmark_results.PublicationPlan, release_tag: str) -> str:
    if release_tag != plan.release_tag:
        raise benchmark_results.BenchmarkResultError(
            f"release tag {release_tag!r} does not match {plan.release_tag!r}"
        )
    head = git_output("rev-parse", "HEAD")
    tag_target = git_output("rev-parse", f"{release_tag}^{{commit}}")
    if head != tag_target:
        raise benchmark_results.BenchmarkResultError(
            f"release tag target {tag_target} does not match checked-out HEAD {head}"
        )
    return head


def read_local_run(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise benchmark_results.BenchmarkResultError(
            f"cannot read local benchmark run: {path}"
        ) from error
    if not isinstance(value, dict):
        raise benchmark_results.BenchmarkResultError(
            "local benchmark run root must be an object"
        )
    return value


def write_index(plan: benchmark_results.PublicationPlan) -> None:
    INDEX_PATH.parent.mkdir(parents=True, exist_ok=True)
    INDEX_PATH.write_text(
        benchmark_retention.render_index(load_retention_plan()),
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    build_arguments = (args.input, args.output_dir, args.release_tag)
    if args.build_assets != all(argument is not None for argument in build_arguments):
        print(
            "--input, --output-dir, and --release-tag are required exactly with --build-assets",
            file=sys.stderr,
        )
        return 2
    if not args.build_assets and any(argument is not None for argument in build_arguments):
        print(
            "asset arguments are accepted only with --build-assets",
            file=sys.stderr,
        )
        return 2
    if args.source_revision is not None and not args.build_assets:
        print("--source-revision is accepted only with --build-assets", file=sys.stderr)
        return 2
    try:
        plan = load_plan()
        if args.write_index:
            write_index(plan)
            print(f"Wrote {INDEX_PATH.relative_to(REPO_ROOT)}")
            return 0
        if args.check:
            check_plan(plan)
            print(
                f"Benchmark result publication OK: {plan.release_tag}, "
                "3 exact release assets"
            )
            return 0

        assert args.input is not None
        assert args.output_dir is not None
        assert args.release_tag is not None
        tagged_revision = exact_tag_revision(plan, args.release_tag)
        if args.source_revision is not None and args.source_revision != tagged_revision:
            raise benchmark_results.BenchmarkResultError(
                "explicit source revision does not match the exact tag target"
            )
        assets = benchmark_results.write_release_assets(
            plan,
            read_local_run(args.input),
            args.output_dir,
            release_tag=args.release_tag,
            source_revision=tagged_revision,
        )
        print(
            f"Wrote {assets.raw_path}, {assets.report_path}, and "
            f"{assets.checksums_path}; raw SHA-256 {assets.raw_sha256}"
        )
        return 0
    except (
        benchmark_results.BenchmarkResultError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Benchmark result publication error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
