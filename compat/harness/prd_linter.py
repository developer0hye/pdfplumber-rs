"""Structural contracts for the PRD master checklist and Evidence Ledger."""

from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass


SECTION_PATTERN = re.compile(r"^##\s+(8|9|13|14)\.(?:\s|$)")
TASK_PATTERN = re.compile(
    r"^\s*-\s+\[([ xX])\]\s+\*\*([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)\*\*"
)
EVIDENCE_PATTERN = re.compile(
    r"^\s*\|\s*`([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+)`\s*\|"
)


class PrdLintError(ValueError):
    """Raised when the PRD violates a structural contract."""


@dataclass(frozen=True)
class Task:
    identifier: str
    checked: bool
    line: int


@dataclass(frozen=True)
class LintResult:
    task_count: int
    checked_count: int
    evidence_count: int


def lint_document(text: str) -> LintResult:
    """Validate task uniqueness and evidence coverage in one PRD document."""

    lines = text.splitlines()
    fenced_lines = _find_fenced_lines(lines)
    sections = _find_contract_sections(lines, fenced_lines)
    tasks = _parse_tasks(lines, sections["8"], sections["9"], fenced_lines)
    evidence = _parse_evidence(
        lines, sections["13"], sections["14"], fenced_lines
    )

    problems: list[str] = []
    task_lines: dict[str, list[int]] = defaultdict(list)
    for task in tasks:
        task_lines[task.identifier].append(task.line)
    for identifier, source_lines in sorted(task_lines.items()):
        if len(source_lines) > 1:
            joined_lines = ", ".join(str(line) for line in source_lines)
            problems.append(
                f"duplicate task identifier {identifier} at lines {joined_lines}"
            )

    evidence_ids = {identifier for identifier, _line in evidence}
    for task in tasks:
        if task.checked and task.identifier not in evidence_ids:
            problems.append(
                f"checked task {task.identifier} at line {task.line} "
                "has no Evidence Ledger row"
            )

    if problems:
        raise PrdLintError("; ".join(problems))

    return LintResult(
        task_count=len(tasks),
        checked_count=sum(task.checked for task in tasks),
        evidence_count=len(evidence),
    )


def _find_fenced_lines(lines: list[str]) -> set[int]:
    fenced_lines: set[int] = set()
    fence_character: str | None = None
    minimum_length = 0

    for index, line in enumerate(lines):
        stripped = line.lstrip(" ")
        if len(line) - len(stripped) > 3:
            if fence_character is not None:
                fenced_lines.add(index)
            continue

        if fence_character is None:
            match = re.match(r"(`{3,}|~{3,})", stripped)
            if match:
                marker = match.group(1)
                fence_character = marker[0]
                minimum_length = len(marker)
                fenced_lines.add(index)
            continue

        fenced_lines.add(index)
        closing = re.fullmatch(
            rf"{re.escape(fence_character)}{{{minimum_length},}}\s*", stripped
        )
        if closing:
            fence_character = None
            minimum_length = 0

    return fenced_lines


def _find_contract_sections(
    lines: list[str], fenced_lines: set[int]
) -> dict[str, int]:
    occurrences: dict[str, list[int]] = {
        number: [] for number in ("8", "9", "13", "14")
    }
    for index, line in enumerate(lines):
        if index in fenced_lines:
            continue
        match = SECTION_PATTERN.match(line)
        if match:
            occurrences[match.group(1)].append(index)

    problems: list[str] = []
    for number, indexes in occurrences.items():
        if not indexes:
            problems.append(f"missing section {number}")
        elif len(indexes) > 1:
            problems.append(f"section {number} appears {len(indexes)} times")
    if problems:
        raise PrdLintError("; ".join(problems))

    sections = {number: indexes[0] for number, indexes in occurrences.items()}
    if not (
        sections["8"] < sections["9"] < sections["13"] < sections["14"]
    ):
        raise PrdLintError("sections 8, 9, 13, and 14 are out of order")
    return sections


def _parse_tasks(
    lines: list[str], start: int, end: int, fenced_lines: set[int]
) -> list[Task]:
    tasks: list[Task] = []
    for index in range(start + 1, end):
        if index in fenced_lines:
            continue
        match = TASK_PATTERN.match(lines[index])
        if match:
            tasks.append(
                Task(
                    identifier=match.group(2),
                    checked=match.group(1).lower() == "x",
                    line=index + 1,
                )
            )
    if not tasks:
        raise PrdLintError("section 8 contains no task definitions")
    return tasks


def _parse_evidence(
    lines: list[str], start: int, end: int, fenced_lines: set[int]
) -> list[tuple[str, int]]:
    rows: list[tuple[str, int]] = []
    for index in range(start + 1, end):
        if index in fenced_lines:
            continue
        match = EVIDENCE_PATTERN.match(lines[index])
        if match:
            rows.append((match.group(1), index + 1))
    return rows
