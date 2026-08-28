#!/usr/bin/env python3
"""Generate or validate the versioned human compatibility workflow report."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from compat.harness import (
    compatibility_scorecard,
    corpus_index,
    workflow_scorecard,
)

DEFAULT_SOURCE = REPO_ROOT / "compat" / "workflow-scorecard-v0.3.0.toml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="reject a stale report")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    return parser.parse_args()


def load_toml(path: Path) -> Mapping[str, object]:
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise workflow_scorecard.WorkflowScorecardError(
            f"cannot read workflow source {path}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise workflow_scorecard.WorkflowScorecardError(
            f"workflow source is not a table: {path}"
        )
    return value


def load_json(path: Path) -> Mapping[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise workflow_scorecard.WorkflowScorecardError(
            f"cannot read machine scorecard {path}: {error}"
        ) from error
    if not isinstance(value, dict):
        raise workflow_scorecard.WorkflowScorecardError(
            f"machine scorecard is not an object: {path}"
        )
    return value


def repository_path(value: object, context: str) -> Path:
    if not isinstance(value, str) or not value:
        raise workflow_scorecard.WorkflowScorecardError(
            f"{context} must be a non-empty repository-relative path"
        )
    configured = Path(value)
    if configured.is_absolute():
        raise workflow_scorecard.WorkflowScorecardError(
            f"{context} must be repository-relative"
        )
    resolved = (REPO_ROOT / configured).resolve()
    try:
        resolved.relative_to(REPO_ROOT.resolve())
    except ValueError as error:
        raise workflow_scorecard.WorkflowScorecardError(
            f"{context} escapes the repository"
        ) from error
    return resolved


def workflow_definitions(
    source: Mapping[str, object],
) -> tuple[workflow_scorecard.WorkflowDefinition, ...]:
    values = source.get("workflows")
    if not isinstance(values, list):
        raise workflow_scorecard.WorkflowScorecardError(
            "workflow source workflows must be an array of tables"
        )
    definitions: list[workflow_scorecard.WorkflowDefinition] = []
    for position, value in enumerate(values):
        if not isinstance(value, dict):
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} must be a table"
            )
        unexpected = set(value) - {
            "id",
            "title",
            "apis",
            "projection",
            "not_tested_reason",
        }
        if unexpected:
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} has unknown fields: {sorted(unexpected)}"
            )
        api_values = value.get("apis", [])
        if not isinstance(api_values, list) or not all(
            isinstance(api, str) and api for api in api_values
        ):
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} apis must be strings"
            )
        identifier = value.get("id")
        title = value.get("title")
        projection = value.get("projection")
        reason = value.get("not_tested_reason")
        if not isinstance(identifier, str) or not identifier:
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} needs an id"
            )
        if not isinstance(title, str) or not title:
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} needs a title"
            )
        if projection is not None and not isinstance(projection, str):
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} projection must be a string"
            )
        if reason is not None and not isinstance(reason, str):
            raise workflow_scorecard.WorkflowScorecardError(
                f"workflow source entry {position} reason must be a string"
            )
        definitions.append(
            workflow_scorecard.WorkflowDefinition(
                identifier=identifier,
                title=title,
                api_ids=tuple(api_values),
                projection=projection,
                not_tested_reason=reason,
            )
        )
    return tuple(definitions)


def main() -> int:
    args = parse_args()
    try:
        source_path = args.source.resolve()
        source = load_toml(source_path)
        if source.get("schema_version") != workflow_scorecard.SCHEMA_VERSION:
            raise workflow_scorecard.WorkflowScorecardError(
                "workflow source schema_version must be 1"
            )
        machine_path = repository_path(
            source.get("machine_scorecard"),
            "machine_scorecard",
        )
        output_path = repository_path(source.get("output"), "output")
        if output_path.suffix != ".md":
            raise workflow_scorecard.WorkflowScorecardError(
                "workflow output must be Markdown"
            )
        machine_bytes = machine_path.read_bytes()
        machine = load_json(machine_path)
        machine_corpus = machine.get("corpus")
        if not isinstance(machine_corpus, dict):
            raise workflow_scorecard.WorkflowScorecardError(
                "machine scorecard corpus must be an object"
            )
        corpus_path = repository_path(machine_corpus.get("index"), "corpus index")
        corpus = corpus_index.load_index(corpus_path)
        corpus_digest = hashlib.sha256(corpus_path.read_bytes()).hexdigest()
        compatibility_scorecard.validate(
            machine,
            corpus=corpus,
            corpus_sha256=corpus_digest,
        )
        report = workflow_scorecard.build(
            machine,
            workflow_definitions(source),
            machine_path=machine_path.relative_to(REPO_ROOT).as_posix(),
            machine_sha256=hashlib.sha256(machine_bytes).hexdigest(),
            indexed_fixture_ids=tuple(fixture.path for fixture in corpus.fixtures),
        )
        rendered = workflow_scorecard.render(report)
        if args.check:
            try:
                committed = output_path.read_text(encoding="utf-8")
            except OSError as error:
                raise workflow_scorecard.WorkflowScorecardError(
                    f"cannot read committed workflow report: {error}"
                ) from error
            if committed != rendered:
                raise workflow_scorecard.WorkflowScorecardError(
                    f"workflow report is stale: {output_path.relative_to(REPO_ROOT)}"
                )
            print(
                "Compatibility workflow report is current: "
                f"{output_path.relative_to(REPO_ROOT)}"
            )
            return 0

        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(rendered, encoding="utf-8")
        print(f"Wrote compatibility workflow report: {output_path.relative_to(REPO_ROOT)}")
        return 0
    except (
        OSError,
        compatibility_scorecard.ScorecardError,
        corpus_index.CorpusIndexError,
        workflow_scorecard.WorkflowScorecardError,
    ) as error:
        print(f"workflow scorecard failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
