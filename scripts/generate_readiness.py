#!/usr/bin/env python3
"""Generate or verify the versioned public readiness page."""

from __future__ import annotations

import argparse
import ast
import difflib
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import tomllib

try:
    from release_version import ReleaseVersionError, load_release_identity
except ModuleNotFoundError:
    from scripts.release_version import ReleaseVersionError, load_release_identity

REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_PATH = REPO_ROOT / "readiness.toml"
OUTPUT_DIR = REPO_ROOT / "docs" / "readiness"
SURFACE_IDS = ("rust", "python", "cli", "wasm")
MILESTONE_IDS = ("M0", "M1", "M2", "M3", "M4")
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
ADOPTION_TASK_PATTERN = re.compile(
    r"^- \[([ xX])\] \*\*(ADOPT-[0-9]{3})\*\*", re.MULTILINE
)
MILESTONE_HEADING_PATTERN = re.compile(r"^### (M[0-4]) — (.+)$", re.MULTILINE)
CHECKLIST_ITEM_PATTERN = re.compile(r"^- \[([ xX])\] (.+)$", re.MULTILINE)


class ReadinessError(ValueError):
    """Raised when readiness inputs violate their generated-page contract."""


@dataclass(frozen=True)
class TestContract:
    identifier: str
    path: str
    label: str


@dataclass(frozen=True)
class Workflow:
    identifier: str
    surface_name: str
    maturity: str
    name: str
    task_ids: tuple[str, ...]
    tests: tuple[TestContract, ...]


@dataclass(frozen=True)
class Milestone:
    identifier: str
    name: str
    checked: int
    total: int


@dataclass(frozen=True)
class Readiness:
    release_version: str
    support_source: str
    milestone_source: str
    workflows: tuple[Workflow, ...]
    milestones: tuple[Milestone, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the versioned readiness page is missing or stale",
    )
    return parser.parse_args()


def require_string(mapping: dict[str, Any], key: str, context: str) -> str:
    value = mapping.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ReadinessError(f"{context}.{key} must be a non-empty string")
    return value


def require_string_list(
    mapping: dict[str, Any], key: str, context: str
) -> tuple[str, ...]:
    value = mapping.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item.strip() for item in value)
    ):
        raise ReadinessError(f"{context}.{key} must be a non-empty string list")
    if len(value) != len(set(value)):
        raise ReadinessError(f"{context}.{key} contains duplicates")
    return tuple(value)


def repository_file(value: str, context: str) -> Path:
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise ReadinessError(f"{context} must be a safe repository-relative path")
    path = REPO_ROOT / relative
    if not path.is_file():
        raise ReadinessError(f"{context} does not exist: {value}")
    return path


def load_toml(path: Path, context: str) -> dict[str, Any]:
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ReadinessError(f"cannot read {context}: {error}") from error
    if not isinstance(value, dict):
        raise ReadinessError(f"{context} must contain a TOML table")
    return value


def adoption_task_states(prd: str) -> dict[str, bool]:
    start_marker = "<!-- ADOPTION-TASKS:START -->"
    end_marker = "<!-- ADOPTION-TASKS:END -->"
    if prd.count(start_marker) != 1 or prd.count(end_marker) != 1:
        raise ReadinessError("PRD must contain one adoption-task marker pair")
    section = prd.split(start_marker, 1)[1].split(end_marker, 1)[0]
    states: dict[str, bool] = {}
    for match in ADOPTION_TASK_PATTERN.finditer(section):
        identifier = match.group(2)
        if identifier in states:
            raise ReadinessError(f"duplicate adoption task {identifier}")
        states[identifier] = match.group(1).lower() == "x"
    if not states:
        raise ReadinessError("PRD adoption-task section contains no tasks")
    return states


def parse_milestones(prd: str) -> tuple[Milestone, ...]:
    start_marker = "## 11. Milestone Gates"
    end_marker = "## 12. Active Work"
    if prd.count(start_marker) != 1 or prd.count(end_marker) != 1:
        raise ReadinessError("PRD must contain one milestone section")
    section = prd.split(start_marker, 1)[1].split(end_marker, 1)[0]
    headings = list(MILESTONE_HEADING_PATTERN.finditer(section))
    identifiers = tuple(match.group(1) for match in headings)
    if identifiers != MILESTONE_IDS:
        raise ReadinessError(
            f"milestone ids/order must be {MILESTONE_IDS}, got {identifiers}"
        )

    milestones: list[Milestone] = []
    for index, heading in enumerate(headings):
        end = headings[index + 1].start() if index + 1 < len(headings) else len(section)
        body = section[heading.end() : end]
        items = list(CHECKLIST_ITEM_PATTERN.finditer(body))
        if not items:
            raise ReadinessError(f"milestone {heading.group(1)} has no checklist items")
        milestones.append(
            Milestone(
                identifier=heading.group(1),
                name=heading.group(2),
                checked=sum(item.group(1).lower() == "x" for item in items),
                total=len(items),
            )
        )
    return tuple(milestones)


def resolve_test_contract(identifier: str) -> TestContract:
    try:
        module_name, class_name, method_name = identifier.rsplit(".", 2)
    except ValueError as error:
        raise ReadinessError(f"invalid test identifier: {identifier}") from error
    if not module_name.startswith("compat.tests.") or not method_name.startswith(
        "test_"
    ):
        raise ReadinessError(f"unsupported test identifier: {identifier}")

    relative = Path(*module_name.split(".")).with_suffix(".py")
    path = repository_file(relative.as_posix(), f"test {identifier}")
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(relative))
    except (OSError, UnicodeError, SyntaxError) as error:
        raise ReadinessError(f"cannot inspect test {identifier}: {error}") from error

    class_node = next(
        (
            node
            for node in tree.body
            if isinstance(node, ast.ClassDef) and node.name == class_name
        ),
        None,
    )
    if class_node is None:
        raise ReadinessError(f"test class does not exist: {identifier}")
    if not any(
        isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name == method_name
        for node in class_node.body
    ):
        raise ReadinessError(f"test method does not exist: {identifier}")

    return TestContract(
        identifier=identifier,
        path=relative.as_posix(),
        label=f"{class_name}.{method_name}",
    )


def load_readiness(path: Path = SOURCE_PATH) -> Readiness:
    release = load_release_identity(REPO_ROOT)
    source = load_toml(path, path.relative_to(REPO_ROOT).as_posix())
    if source.get("schema_version") != 1:
        raise ReadinessError("schema_version must be 1")

    release_version = require_string(source, "release_version", "readiness")
    if not VERSION_PATTERN.fullmatch(release_version):
        raise ReadinessError("readiness.release_version must be a semantic version")
    if release_version != release.version:
        raise ReadinessError(
            f"readiness release {release_version} != workspace {release.version}"
        )
    support_source = require_string(source, "support_source", "readiness")
    milestone_source = require_string(source, "milestone_source", "readiness")
    support_path = repository_file(support_source, "readiness.support_source")
    milestone_path = repository_file(milestone_source, "readiness.milestone_source")

    support = load_toml(support_path, support_source)
    if support.get("release_version") != release_version:
        raise ReadinessError(
            f"readiness release {release_version} != support release "
            f"{support.get('release_version')}"
        )
    surfaces = support.get("surfaces")
    if not isinstance(surfaces, list):
        raise ReadinessError("support surfaces must be an array of tables")
    surface_map = {
        surface.get("id"): surface for surface in surfaces if isinstance(surface, dict)
    }
    if tuple(surface_map) != SURFACE_IDS:
        raise ReadinessError(
            f"support surface ids/order must be {SURFACE_IDS}, got {tuple(surface_map)}"
        )

    prd = milestone_path.read_text(encoding="utf-8")
    task_states = adoption_task_states(prd)
    milestones = parse_milestones(prd)

    workflow_values = source.get("workflows")
    if not isinstance(workflow_values, list):
        raise ReadinessError("workflows must be an array of tables")
    identifiers = tuple(
        workflow.get("id") for workflow in workflow_values if isinstance(workflow, dict)
    )
    if identifiers != SURFACE_IDS:
        raise ReadinessError(
            f"workflow ids/order must be {SURFACE_IDS}, got {identifiers}"
        )

    workflows: list[Workflow] = []
    for workflow in workflow_values:
        identifier = require_string(workflow, "id", "workflow")
        context = f"workflow[{identifier}]"
        if "status" in workflow:
            raise ReadinessError(f"{context}.status is derived and must not be stored")
        name = require_string(workflow, "name", context)
        task_ids = require_string_list(workflow, "task_ids", context)
        test_ids = require_string_list(workflow, "test_ids", context)

        unknown_tasks = [task for task in task_ids if task not in task_states]
        unchecked_tasks = [
            task for task in task_ids if not task_states.get(task, False)
        ]
        if unknown_tasks:
            raise ReadinessError(f"{context} references unknown tasks: {unknown_tasks}")
        if unchecked_tasks:
            raise ReadinessError(
                f"{context} references unchecked tasks: {unchecked_tasks}"
            )

        surface = surface_map[identifier]
        surface_name = require_string(surface, "name", f"surface[{identifier}]")
        maturity = require_string(surface, "maturity", f"surface[{identifier}]")
        workflows.append(
            Workflow(
                identifier=identifier,
                surface_name=surface_name,
                maturity=maturity,
                name=name,
                task_ids=task_ids,
                tests=tuple(resolve_test_contract(test_id) for test_id in test_ids),
            )
        )

    return Readiness(
        release_version=release_version,
        support_source=support_source,
        milestone_source=milestone_source,
        workflows=tuple(workflows),
        milestones=milestones,
    )


def escape_cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def render(readiness: Readiness) -> str:
    lines = [
        f"# What is ready today? — v{readiness.release_version}",
        "",
        "<!-- Generated by `scripts/generate_readiness.py` from `readiness.toml`, `support-matrix.toml`, `PRD.md`, and named test contracts. Do not edit this file directly. -->",
        "",
        "This versioned snapshot reports only workflows whose adoption tasks are checked and whose named compatibility tests exist. `CI-contracted` means the complete compatibility harness executes those contracts on every change; it does not promote a surface beyond the maturity in the generated support matrix.",
        "",
        "## Ready workflows",
        "",
        "| Surface | Maturity | Status | Workflow | Checked tasks | Test contracts |",
        "|---|---|---|---|---|---|",
    ]

    for workflow in readiness.workflows:
        tasks = "<br>".join(
            f"[`{task_id}`](../../PRD.md)" for task_id in workflow.task_ids
        )
        tests = "<br>".join(
            f"[`{test.label}`](../../{test.path})" for test in workflow.tests
        )
        lines.append(
            "| "
            + " | ".join(
                (
                    escape_cell(workflow.surface_name),
                    workflow.maturity.title(),
                    "CI-contracted",
                    escape_cell(workflow.name),
                    tasks,
                    tests,
                )
            )
            + " |"
        )

    lines.extend(
        (
            "",
            "## Milestone state",
            "",
            "Milestone progress is read directly from the M0–M4 base checklists in `PRD.md`. The adoption gates in section 11.0 also apply, so a complete base checklist alone does not authorize a maturity promotion.",
            "",
            "| Milestone | Name | Base checklist | State |",
            "|---|---|---|---|",
        )
    )
    for milestone in readiness.milestones:
        state = (
            "Base checklist complete"
            if milestone.checked == milestone.total
            else "Base checklist in progress"
        )
        lines.append(
            f"| {milestone.identifier} | {escape_cell(milestone.name)} | "
            f"{milestone.checked}/{milestone.total} | {state} |"
        )

    lines.extend(
        (
            "",
            "## Scope boundary",
            "",
            f"- Surface versions, maturity, platforms, features, and limitations come from [`{readiness.support_source}`](../../{readiness.support_source}) and its [generated support page](../support.md).",
            f"- Milestone and adoption-task state comes from [`{readiness.milestone_source}`](../../{readiness.milestone_source}).",
            "- Workflow-to-task and workflow-to-test mappings come from [`readiness.toml`](../../readiness.toml).",
            "- Registry installation after publication, broader platform coverage, full Python drop-in compatibility, browser execution, and cross-project performance claims remain outside this snapshot unless their linked gates become checked.",
            "",
            "## Updating this page",
            "",
            "Update structured task or test mappings only when their repository evidence changes, then run:",
            "",
            "```console",
            "python3 scripts/generate_readiness.py",
            "python3 scripts/generate_readiness.py --check",
            "```",
            "",
            "The generator rejects unchecked or unknown task IDs, missing test methods, release-version drift, support-surface drift, malformed milestone state, and stale rendered output.",
            "",
        )
    )
    return "\n".join(lines)


def output_path(release_version: str) -> Path:
    return OUTPUT_DIR / f"v{release_version}.md"


def main() -> int:
    args = parse_args()
    try:
        readiness = load_readiness()
        generated = render(readiness)
    except (ReadinessError, ReleaseVersionError) as error:
        print(f"readiness validation failed: {error}", file=sys.stderr)
        return 1

    output = output_path(readiness.release_version)
    relative_output = output.relative_to(REPO_ROOT)
    if args.check:
        if not output.is_file():
            print(f"readiness page is missing: {relative_output}")
            return 1
        committed = output.read_text(encoding="utf-8")
        if committed == generated:
            print(f"readiness page is current: {relative_output}")
            return 0
        print(f"readiness page is stale: {relative_output}")
        diff = difflib.unified_diff(
            committed.splitlines(),
            generated.splitlines(),
            fromfile="committed",
            tofile="generated",
            lineterm="",
        )
        for line in list(diff)[:200]:
            print(line)
        return 1

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(generated, encoding="utf-8")
    print(f"wrote readiness page: {relative_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
