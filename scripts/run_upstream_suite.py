#!/usr/bin/env python3
"""Run pinned upstream tests in the isolated installed-candidate environment."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import upstream, upstream_suite  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--python",
        type=Path,
        default=REPO_ROOT / ".venv-candidate" / "bin" / "python",
        help="Python executable containing the installed compatibility package",
    )
    parser.add_argument(
        "--tests",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-tests",
    )
    parser.add_argument(
        "--source-manifest",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-suite.toml",
    )
    parser.add_argument(
        "--unsupported-manifest",
        type=Path,
        default=REPO_ROOT / "compat" / "upstream-unsupported.toml",
    )
    parser.add_argument(
        "--result",
        type=Path,
        default=REPO_ROOT / "target" / "upstream-suite-result.json",
    )
    parser.add_argument("--workers", default="auto")
    parser.add_argument("pytest_args", nargs=argparse.REMAINDER)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    candidate_python = args.python.absolute()
    try:
        config = upstream_suite.load_source_manifest(args.source_manifest)
        manifest = upstream_suite.load_unsupported_manifest(
            args.unsupported_manifest
        )
        verify_target(config, manifest)
        verify_requirements(config)
        upstream_suite.verify_materialized_suite(args.tests, config)
        candidate = candidate_preflight(candidate_python)
        verify_test_dependencies(candidate_python)
        verify_external_commands(config)
    except (
        OSError,
        subprocess.CalledProcessError,
        upstream_suite.SuiteSourceMismatch,
        upstream_suite.UnsupportedManifestError,
        ValueError,
    ) as error:
        print(f"upstream suite preflight failed: {error}", file=sys.stderr)
        print("Prepare source: python3 scripts/setup_upstream_suite.py", file=sys.stderr)
        print(
            "Install test tools: .venv-candidate/bin/python -m pip install "
            "--require-hashes -r compat/requirements-upstream-tests.txt",
            file=sys.stderr,
        )
        return 1

    args.result.parent.mkdir(parents=True, exist_ok=True)
    args.result.unlink(missing_ok=True)
    environment = os.environ.copy()
    environment["PYTHONNOUSERSITE"] = "1"
    environment["PYTHONPATH"] = str(REPO_ROOT / "compat" / "harness")
    environment["PDFPLUMBER_EXPECTED_CANDIDATE_ORIGIN"] = candidate["origin"]
    environment["PDFPLUMBER_UPSTREAM_RESULTS"] = str(args.result.resolve())
    command = [
        str(candidate_python),
        "-m",
        "pytest",
        "-n",
        args.workers,
        "-p",
        "upstream_pytest_plugin",
        ".",
        *args.pytest_args,
    ]
    completed = subprocess.run(
        command,
        cwd=args.tests / "tests",
        env=environment,
        check=False,
    )
    if not args.result.is_file():
        print(
            "upstream pytest did not produce its machine-readable result; "
            f"pytest exited {completed.returncode}",
            file=sys.stderr,
        )
        return completed.returncode or 1

    raw_result = json.loads(args.result.read_text(encoding="utf-8"))
    classification = upstream_suite.classify_results(
        collected=tuple(raw_result["collected"]),
        failed=tuple(raw_result["failed"]),
        manifest=manifest,
        pytest_exit_code=completed.returncode,
    )
    raw_result.update(
        {
            "candidate": candidate,
            "source": {
                "commit": config.commit,
                "tests_tree": config.tests_tree,
                "tests_sha256": config.tests_sha256,
            },
            "unsupported_manifest": str(
                args.unsupported_manifest.relative_to(REPO_ROOT)
            ),
            "classification": {
                "known_unsupported": list(classification.known_unsupported),
                "unlisted_failures": list(classification.unlisted_failures),
                "stale_unsupported": list(classification.stale_unsupported),
                "uncollected_unsupported": list(
                    classification.uncollected_unsupported
                ),
            },
        }
    )
    args.result.write_text(
        json.dumps(raw_result, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        f"Upstream suite: {len(raw_result['collected'])} collected, "
        f"{len(raw_result['failed'])} failed, "
        f"{len(classification.known_unsupported)} listed unsupported, "
        f"{len(classification.unlisted_failures)} unlisted failures"
    )
    print(f"Result: {args.result.relative_to(REPO_ROOT)}")
    return classification.exit_code


def verify_target(
    config: upstream_suite.SourceConfig,
    manifest: upstream_suite.UnsupportedManifest,
) -> None:
    target = upstream.load_target()
    actual = (
        config.project,
        config.version,
        config.tag,
        config.commit,
        config.repository,
    )
    expected = (
        target.project,
        target.version,
        target.tag,
        target.commit,
        target.repository,
    )
    if actual != expected:
        raise ValueError("upstream-suite source manifest does not match upstream.toml")
    if (manifest.version, manifest.commit) != (target.version, target.commit):
        raise ValueError("unsupported-test manifest does not match upstream.toml")


def verify_requirements(config: upstream_suite.SourceConfig) -> None:
    requirements = REPO_ROOT / config.requirements_path
    digest = hashlib.sha256(requirements.read_bytes()).hexdigest()
    if digest != config.requirements_sha256:
        raise ValueError(
            f"upstream-test requirements digest {digest} does not match "
            f"{config.requirements_sha256}"
        )


def candidate_preflight(python: Path) -> dict[str, str]:
    if not python.is_file():
        raise ValueError(f"candidate Python does not exist: {python}")
    code = (
        "import json, pathlib, pdfplumber; "
        "print(json.dumps({'origin': str(pathlib.Path(pdfplumber.__file__).resolve()), "
        "'version': str(getattr(pdfplumber, '__version__', 'unknown'))}))"
    )
    completed = subprocess.run(
        [str(python), "-I", "-c", code],
        cwd=python.absolute().parent.parent,
        check=True,
        capture_output=True,
        text=True,
    )
    candidate = json.loads(completed.stdout)
    origin = Path(candidate["origin"])
    environment_root = python.absolute().parent.parent.resolve()
    if environment_root not in origin.parents:
        raise ValueError(
            f"candidate pdfplumber was imported from {origin}, "
            f"outside {environment_root}"
        )
    target = upstream.load_target()
    if candidate["version"] == target.version and origin.suffix == ".py":
        raise ValueError(
            f"candidate imported upstream pdfplumber {target.version} at {origin}"
        )
    return candidate


def verify_test_dependencies(python: Path) -> None:
    subprocess.run(
        [str(python), "-I", "-c", "import pandas, pytest, xdist"],
        cwd=python.absolute().parent.parent,
        check=True,
        capture_output=True,
        text=True,
    )


def verify_external_commands(config: upstream_suite.SourceConfig) -> None:
    missing = [
        command for command in config.external_commands if shutil.which(command) is None
    ]
    if missing:
        raise ValueError(
            "missing external upstream-test commands: " + ", ".join(missing)
        )


if __name__ == "__main__":
    raise SystemExit(main())
