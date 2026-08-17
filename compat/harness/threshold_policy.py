"""Fail-closed policy for compatibility percentage-threshold reductions."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Mapping, Sequence


_NUMBER = r"(?:0(?:\.\d+)?|1(?:\.0+)?)"
_TOKEN = rf"(?:{_NUMBER}|[A-Z][A-Z0-9_]*)"
_RUST_CONSTANT = re.compile(
    rf"\bconst\s+(?P<name>[A-Z][A-Z0-9_]*THRESHOLD)\s*:[^=;\n]+="
    rf"\s*(?P<value>{_NUMBER})\s*;"
)
_PYTHON_CONSTANT = re.compile(
    rf"(?m)^(?P<name>[A-Z][A-Z0-9_]*THRESHOLD)"
    rf"\s*(?::[^=\n]+)?=\s*(?P<value>{_NUMBER})\s*$"
)
_RUST_FUNCTION = re.compile(r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)[^;{]*\{")
_COMPARISON = re.compile(
    rf"(?P<lhs>[A-Za-z_][A-Za-z0-9_]*"
    rf"(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*(?:\s*\(\s*\))?)*)"
    rf"\s*>=?\s*(?P<token>{_TOKEN})\b"
)
_CROSS_VALIDATE = re.compile(r"\bcross_validate!\s*\(")
_GUARDED_LHS_TERMS = ("accuracy", "f1", "match", "rate", "score")
_GUARDED_LHS_NAMES = {"acc"}
_LEDGER_START = "## 13. Evidence Ledger"
_LEDGER_END = "## 14. Decision Log"


class ThresholdPolicyError(ValueError):
    """Raised when a threshold reduction does not satisfy the policy."""


class ThresholdValue(float):
    """A numeric threshold retaining the named constant that supplied it."""

    origin: str | None

    def __new__(cls, value: float, origin: str | None = None) -> "ThresholdValue":
        instance = super().__new__(cls, value)
        instance.origin = origin
        return instance


@dataclass(frozen=True)
class Reduction:
    key: str
    before: float
    after: float | None


@dataclass(frozen=True)
class PolicyResult:
    reductions: tuple[Reduction, ...]
    approver: str | None


def _mask_non_code(source: str, *, python_source: bool) -> str:
    """Replace comments and quoted strings while retaining layout and delimiters."""

    result = list(source)
    index = 0
    block_depth = 0
    string_end: str | None = None
    regular_string = False

    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""

        if string_end is not None:
            if regular_string and char == "\\":
                result[index] = " "
                if index + 1 < len(source):
                    if source[index + 1] != "\n":
                        result[index + 1] = " "
                    index += 2
                    continue
            if source.startswith(string_end, index):
                for offset in range(len(string_end)):
                    result[index + offset] = " "
                index += len(string_end)
                string_end = None
                regular_string = False
                continue
            if char != "\n":
                result[index] = " "
            index += 1
            continue

        if block_depth:
            if char == "/" and following == "*":
                result[index] = result[index + 1] = " "
                block_depth += 1
                index += 2
                continue
            if char == "*" and following == "/":
                result[index] = result[index + 1] = " "
                block_depth -= 1
                index += 2
                continue
            if char != "\n":
                result[index] = " "
            index += 1
            continue

        if char == "/" and following == "*":
            result[index] = result[index + 1] = " "
            block_depth = 1
            index += 2
            continue
        if char == "/" and following == "/":
            while index < len(source) and source[index] != "\n":
                result[index] = " "
                index += 1
            continue
        if python_source and char == "#":
            while index < len(source) and source[index] != "\n":
                result[index] = " "
                index += 1
            continue

        if python_source and source.startswith(('"""', "'''"), index):
            string_end = source[index : index + 3]
            for offset in range(3):
                result[index + offset] = " "
            index += 3
            continue

        if not python_source and char == "r":
            raw_match = re.match(r'r(?P<hashes>#{0,16})"', source[index:])
            if raw_match:
                opening = raw_match.group(0)
                string_end = '"' + raw_match.group("hashes")
                for offset in range(len(opening)):
                    result[index + offset] = " "
                index += len(opening)
                continue

        if char == '"' or (python_source and char == "'"):
            string_end = char
            regular_string = True
            result[index] = " "
            index += 1
            continue

        if not python_source and char == "'":
            # Mask Rust character literals, but do not confuse a lifetime such
            # as `'a` with an unterminated string.
            closing = index + (3 if following == "\\" else 2)
            if closing < len(source) and source[closing] == "'":
                string_end = "'"
                regular_string = True
                result[index] = " "
                index += 1
                continue
        index += 1

    return "".join(result)


def _matching_delimiter(source: str, opening: int, left: str, right: str) -> int:
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == left:
            depth += 1
        elif source[index] == right:
            depth -= 1
            if depth == 0:
                return index
    raise ThresholdPolicyError(f"unbalanced {left}{right} delimiters")


def _split_top_level(arguments: str) -> list[str]:
    parts: list[str] = []
    start = 0
    depths = {"(": 0, "[": 0, "{": 0}
    closing = {")": "(", "]": "[", "}": "{"}

    for index, char in enumerate(arguments):
        if char in depths:
            depths[char] += 1
        elif char in closing:
            depths[closing[char]] -= 1
        elif char == "," and not any(depths.values()):
            parts.append(arguments[start:index].strip())
            start = index + 1
    parts.append(arguments[start:].strip())
    return parts


def _resolve(token: str, constants: Mapping[str, float]) -> ThresholdValue | None:
    token = token.strip()
    if token in constants:
        return ThresholdValue(constants[token], origin=token)
    try:
        value = float(token)
    except ValueError:
        return None
    return ThresholdValue(value) if 0.0 <= value <= 1.0 else None


def _normalise_lhs(lhs: str) -> str:
    return re.sub(r"\s+", "", lhs).replace("()", "")


def _is_guarded_lhs(lhs: str) -> bool:
    lowered = lhs.lower()
    final_name = lowered.rsplit(".", 1)[-1]
    return final_name in _GUARDED_LHS_NAMES or any(
        term in final_name for term in _GUARDED_LHS_TERMS
    )


def extract_thresholds(path: str, source: str) -> dict[str, float]:
    """Extract guarded percentage thresholds from Rust or Python source."""

    code = _mask_non_code(source, python_source=path.endswith(".py"))
    thresholds: dict[str, float] = {}
    constants: dict[str, float] = {}

    for pattern in (_RUST_CONSTANT, _PYTHON_CONSTANT):
        for match in pattern.finditer(code):
            name = match.group("name")
            value = ThresholdValue(float(match.group("value")))
            constants[name] = value
            thresholds[f"{path}::const::{name}"] = value

    for match in _CROSS_VALIDATE.finditer(code):
        opening = match.end() - 1
        closing = _matching_delimiter(code, opening, "(", ")")
        arguments = _split_top_level(code[opening + 1 : closing])
        if len(arguments) < 4:
            continue
        case_name = arguments[0]
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", case_name):
            continue
        for dimension, token in zip(("char", "word"), arguments[2:4]):
            value = _resolve(token, constants)
            if value is not None:
                thresholds[
                    f"{path}::cross_validate::{case_name}::{dimension}"
                ] = value

    for function_match in _RUST_FUNCTION.finditer(code):
        opening = function_match.end() - 1
        closing = _matching_delimiter(code, opening, "{", "}")
        body = code[opening + 1 : closing]
        counts: dict[str, int] = {}
        for comparison in _COMPARISON.finditer(body):
            lhs = _normalise_lhs(comparison.group("lhs"))
            if not _is_guarded_lhs(lhs):
                continue
            value = _resolve(comparison.group("token"), constants)
            if value is None:
                continue
            counts[lhs] = counts.get(lhs, 0) + 1
            key = (
                f"{path}::fn::{function_match.group('name')}::"
                f"{lhs}::{counts[lhs]}"
            )
            thresholds[key] = value

    if path.endswith(".py"):
        counts: dict[str, int] = {}
        for comparison in _COMPARISON.finditer(code):
            lhs = _normalise_lhs(comparison.group("lhs"))
            if not _is_guarded_lhs(lhs):
                continue
            value = _resolve(comparison.group("token"), constants)
            if value is None:
                continue
            counts[lhs] = counts.get(lhs, 0) + 1
            thresholds[f"{path}::python::{lhs}::{counts[lhs]}"] = value

    return dict(sorted(thresholds.items()))


def find_reductions(
    before: Mapping[str, float], after: Mapping[str, float]
) -> tuple[Reduction, ...]:
    """Return every lowered or removed guarded threshold."""

    candidates: list[Reduction] = []
    for key, before_value in sorted(before.items()):
        after_value = after.get(key)
        if after_value is None or float(after_value) < float(before_value):
            candidates.append(
                Reduction(
                    key,
                    float(before_value),
                    None if after_value is None else float(after_value),
                )
            )

    reduced_keys = {candidate.key for candidate in candidates}
    reductions: list[Reduction] = []
    for candidate in candidates:
        before_value = before[candidate.key]
        origin = getattr(before_value, "origin", None)
        after_value = after.get(candidate.key)
        after_origin = getattr(after_value, "origin", None)
        path = candidate.key.split("::", 1)[0]
        constant_key = f"{path}::const::{origin}" if origin else ""
        same_origin = after_value is None or after_origin == origin
        if origin and same_origin and constant_key in reduced_keys:
            # A shared constant change is one policy decision, not one decision
            # per dependent assertion. Changing a use to a literal remains an
            # independently reported reduction.
            continue
        reductions.append(candidate)
    return tuple(reductions)


def _format_number(value: float) -> str:
    return format(value, ".12g")


def format_evidence_marker(reduction: Reduction) -> str:
    after = "REMOVED" if reduction.after is None else _format_number(reduction.after)
    return (
        f"THRESHOLD-REDUCTION::{reduction.key}::"
        f"{_format_number(reduction.before)}->{after}"
    )


def _evidence_ledger(prd_text: str) -> str:
    start = prd_text.find(_LEDGER_START)
    end = prd_text.find(_LEDGER_END)
    if start < 0 or end < 0 or end <= start:
        raise ThresholdPolicyError("PRD Evidence Ledger section is missing or malformed")
    return prd_text[start:end]


def _current_maintainer_approver(
    reviews: Sequence[Mapping[str, str]],
    permissions: Mapping[str, str],
    head_sha: str,
) -> str | None:
    decisive_states = {"APPROVED", "CHANGES_REQUESTED", "DISMISSED"}
    latest: dict[str, Mapping[str, str]] = {}
    for review in reviews:
        login = review.get("login", "")
        state = review.get("state", "").upper()
        if login and state in decisive_states:
            latest[login] = review

    for login in sorted(latest):
        review = latest[login]
        if (
            review.get("state", "").upper() == "APPROVED"
            and review.get("commit_id") == head_sha
            and permissions.get(login, "").lower() in {"admin", "maintain"}
        ):
            return login
    return None


def enforce_policy(
    reductions: Sequence[Reduction],
    *,
    prd_text: str,
    reviews: Sequence[Mapping[str, str]],
    permissions: Mapping[str, str],
    head_sha: str,
) -> PolicyResult:
    """Require ledger markers and a current maintainer review for reductions."""

    reductions = tuple(reductions)
    if not reductions:
        return PolicyResult(reductions=(), approver=None)

    ledger = _evidence_ledger(prd_text)
    missing = [
        format_evidence_marker(reduction)
        for reduction in reductions
        if format_evidence_marker(reduction) not in ledger
    ]
    if missing:
        formatted = "\n".join(f"  - {marker}" for marker in missing)
        raise ThresholdPolicyError(
            "percentage-threshold reduction lacks an exact Evidence Ledger marker:\n"
            f"{formatted}"
        )

    approver = _current_maintainer_approver(reviews, permissions, head_sha)
    if approver is None:
        raise ThresholdPolicyError(
            "percentage-threshold reduction lacks maintainer approval: require an "
            "APPROVED review from an admin/maintain collaborator at the exact head SHA"
        )

    return PolicyResult(reductions=reductions, approver=approver)
