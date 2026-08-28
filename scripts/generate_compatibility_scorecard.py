#!/usr/bin/env python3
"""Generate or validate the versioned public compatibility scorecard."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import compatibility_scorecard, corpus_index, upstream

SOURCE_PATH = REPO_ROOT / "compat" / "scorecard-v0.3.0.toml"
CORPUS_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate the committed scorecard without transient raw results",
    )
    parser.add_argument(
        "--parity-report",
        type=Path,
        help="schema-v1 parity report used for every observed run scope",
    )
    return parser.parse_args()


def load_toml(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as source_file:
            value = tomllib.load(source_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise compatibility_scorecard.ScorecardError(
            f"cannot read scorecard source: {path}"
        ) from error
    if not isinstance(value, dict):
        raise compatibility_scorecard.ScorecardError(
            f"scorecard source is not a table: {path}"
        )
    return value


def load_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise compatibility_scorecard.ScorecardError(
            f"cannot read scorecard JSON: {path}"
        ) from error
    if not isinstance(value, dict):
        raise compatibility_scorecard.ScorecardError(
            f"scorecard JSON is not an object: {path}"
        )
    return value


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def json_sha256(value: object) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def output_path(source: Mapping[str, object]) -> Path:
    configured = required_string(source, "output", "scorecard source")
    relative = Path(configured)
    if relative.is_absolute() or ".." in relative.parts:
        raise compatibility_scorecard.ScorecardError(
            f"unsafe scorecard output path: {configured}"
        )
    return REPO_ROOT / relative


def source_runs(source: Mapping[str, object]) -> list[dict[str, object]]:
    value = source.get("runs")
    if not isinstance(value, list) or not value:
        raise compatibility_scorecard.ScorecardError(
            "scorecard source needs a non-empty runs array"
        )
    runs: list[dict[str, object]] = []
    for position, run in enumerate(value):
        if not isinstance(run, dict):
            raise compatibility_scorecard.ScorecardError(
                f"scorecard source run {position} is not a table"
            )
        runs.append(run)
    return runs


def platform_from(run: Mapping[str, object]) -> compatibility_scorecard.Platform:
    run_id = required_string(run, "id", "scorecard source run")
    return compatibility_scorecard.Platform(
        id=required_string(run, "platform_id", run_id),
        system=required_string(run, "platform_system", run_id),
        release=required_string(run, "platform_release", run_id),
        machine=required_string(run, "platform_machine", run_id),
        python_version=required_string(run, "python_version", run_id),
    )


def scopes_from(run: Mapping[str, object]) -> tuple[str, ...]:
    run_id = required_string(run, "id", "scorecard source run")
    value = run.get("scopes")
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(scope, str) for scope in value)
    ):
        raise compatibility_scorecard.ScorecardError(
            f"scorecard source run {run_id} has invalid scopes"
        )
    return tuple(value)


def evidence_from(run: Mapping[str, object]) -> tuple[str, ...]:
    run_id = required_string(run, "id", "scorecard source run")
    value = run.get("evidence", [])
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise compatibility_scorecard.ScorecardError(
            f"scorecard source run {run_id} has invalid evidence"
        )
    return tuple(value)


def build_inputs(
    source: Mapping[str, object],
    report: Mapping[str, object],
) -> tuple[compatibility_scorecard.RunInput, ...]:
    inputs: list[compatibility_scorecard.RunInput] = []
    report_digest = json_sha256(report)
    for run in source_runs(source):
        run_id = required_string(run, "id", "scorecard source run")
        status = required_string(run, "status", run_id)
        common = {
            "id": run_id,
            "platform": platform_from(run),
            "artifact_type": required_string(run, "artifact_type", run_id),
            "scopes": scopes_from(run),
            "evidence": evidence_from(run),
        }
        if status == "observed":
            expected_report_digest = required_string(
                run,
                "parity_report_sha256",
                run_id,
            )
            if expected_report_digest != report_digest:
                raise compatibility_scorecard.ScorecardError(
                    f"run {run_id} parity report SHA-256 is stale"
                )
            inputs.append(
                compatibility_scorecard.RunInput(
                    **common,
                    artifact_name=required_string(run, "artifact_name", run_id),
                    artifact_sha256=required_string(
                        run,
                        "artifact_sha256",
                        run_id,
                    ),
                    toolchain=compatibility_scorecard.Toolchain(
                        rustc=required_string(run, "rustc_version", run_id),
                        cargo=required_string(run, "cargo_version", run_id),
                        builder=required_string(run, "builder_version", run_id),
                    ),
                    command=required_string(run, "command", run_id),
                    report=report,
                )
            )
        elif status == "not_tested":
            inputs.append(
                compatibility_scorecard.RunInput(
                    **common,
                    not_tested_reason=required_string(run, "reason", run_id),
                )
            )
        else:
            raise compatibility_scorecard.ScorecardError(
                f"scorecard source run {run_id} has unknown status {status!r}"
            )
    return tuple(inputs)


def validate_source_and_output(
    source: Mapping[str, object],
    scorecard: Mapping[str, object],
    corpus: corpus_index.CorpusIndex,
    corpus_digest: str,
) -> None:
    if source.get("schema_version") != 1:
        raise compatibility_scorecard.ScorecardError(
            "scorecard source schema_version must be 1"
        )
    if source.get("corpus_sha256") != corpus_digest:
        raise compatibility_scorecard.ScorecardError(
            "scorecard source corpus fingerprint is stale"
        )
    compatibility_scorecard.validate(
        scorecard,
        corpus=corpus,
        corpus_sha256=corpus_digest,
    )
    subject = scorecard["subject"]
    assert isinstance(subject, dict)
    if subject.get("version") != source.get("subject_version"):
        raise compatibility_scorecard.ScorecardError(
            "published scorecard subject version differs from its source"
        )
    if subject.get("revision") != source.get("subject_revision"):
        raise compatibility_scorecard.ScorecardError(
            "published scorecard revision differs from its source"
        )
    target = upstream.load_target()
    expected_target = {
        "project": target.project,
        "version": target.version,
        "tag": target.tag,
        "commit": target.commit,
        "repository": target.repository,
    }
    if scorecard.get("target") != expected_target:
        raise compatibility_scorecard.ScorecardError(
            "published scorecard upstream target is stale"
        )

    published_runs = scorecard.get("runs")
    if not isinstance(published_runs, list):
        raise compatibility_scorecard.ScorecardError(
            "published scorecard runs are invalid"
        )
    published_by_id = {
        required_string(run, "id", "published run"): run
        for run in published_runs
        if isinstance(run, dict)
    }
    configured_runs = source_runs(source)
    if set(published_by_id) != {
        required_string(run, "id", "scorecard source run")
        for run in configured_runs
    }:
        raise compatibility_scorecard.ScorecardError(
            "published scorecard run IDs differ from its source"
        )
    for configured in configured_runs:
        run_id = required_string(configured, "id", "scorecard source run")
        published = published_by_id[run_id]
        for source_key, output_key in (
            ("status", "status"),
            ("platform_id", "platform_id"),
            ("artifact_type", "artifact_type"),
            ("scopes", "scopes"),
            ("evidence", "evidence"),
        ):
            configured_value = configured.get(source_key, [])
            published_value = published.get(output_key, [])
            if configured_value != published_value:
                raise compatibility_scorecard.ScorecardError(
                    f"published run {run_id} {output_key} differs from its source"
                )
        if configured["status"] == "observed":
            for key in (
                "artifact_name",
                "artifact_sha256",
                "command",
                "parity_report_sha256",
            ):
                if configured.get(key) != published.get(key):
                    raise compatibility_scorecard.ScorecardError(
                        f"published run {run_id} {key} differs from its source"
                    )
            expected_toolchain = {
                "rustc": configured.get("rustc_version"),
                "cargo": configured.get("cargo_version"),
                "builder": configured.get("builder_version"),
            }
            if published.get("toolchain") != expected_toolchain:
                raise compatibility_scorecard.ScorecardError(
                    f"published run {run_id} toolchain differs from its source"
                )
        elif configured.get("reason") != published.get("reason"):
            raise compatibility_scorecard.ScorecardError(
                f"published run {run_id} reason differs from its source"
            )


def required_string(data: Mapping[str, object], key: str, context: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise compatibility_scorecard.ScorecardError(
            f"{context} has no non-empty {key}"
        )
    return value


def main() -> int:
    arguments = parse_args()
    try:
        source = load_toml(SOURCE_PATH)
        corpus = corpus_index.load_index(CORPUS_PATH)
        corpus_digest = file_sha256(CORPUS_PATH)
        destination = output_path(source)
        if arguments.check:
            if arguments.parity_report is not None:
                raise compatibility_scorecard.ScorecardError(
                    "--check cannot be combined with --parity-report"
                )
            scorecard = load_json(destination)
            validate_source_and_output(
                source,
                scorecard,
                corpus,
                corpus_digest,
            )
            print(
                f"Compatibility scorecard is current: "
                f"{destination.relative_to(REPO_ROOT)} "
                f"({len(scorecard['observations'])} observations)"
            )
            return 0
        if arguments.parity_report is None:
            raise compatibility_scorecard.ScorecardError(
                "generation requires --parity-report"
            )
        report = load_json(arguments.parity_report)
        scorecard = compatibility_scorecard.build(
            subject_version=required_string(
                source,
                "subject_version",
                "scorecard source",
            ),
            subject_revision=required_string(
                source,
                "subject_revision",
                "scorecard source",
            ),
            corpus=corpus,
            corpus_sha256=corpus_digest,
            runs=build_inputs(source, report),
        )
        validate_source_and_output(
            source,
            scorecard,
            corpus,
            corpus_digest,
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(
            compatibility_scorecard.render(scorecard),
            encoding="utf-8",
        )
        print(
            f"Wrote compatibility scorecard: "
            f"{destination.relative_to(REPO_ROOT)} "
            f"({len(scorecard['observations'])} observations)"
        )
        return 0
    except compatibility_scorecard.ScorecardError as error:
        print(f"compatibility scorecard failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
