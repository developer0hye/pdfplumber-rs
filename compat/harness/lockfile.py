"""Reader for the hash-pinned reference lock file (PARITY-001).

`pip` is the only thing that installs from this file, but the harness needs to
read it too: to prove it is fully pinned before CI trusts it, and to stamp its
digest into every golden artifact so a golden file can be traced back to the
exact dependency set that produced it.

The format is the subset of `requirements.txt` that
`pip install --require-hashes` accepts:

    name==1.2.3 ; python_full_version < '3.11' \\
        --hash=sha256:<64 hex characters> \\
        --hash=sha256:<64 hex characters>
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from pathlib import Path

from compat.harness import upstream

HASH_OPTION: str = "--hash="
_CANONICALIZE_SEPARATORS = re.compile(r"[-_.]+")
_REQUIREMENT = re.compile(
    r"^(?P<name>[A-Za-z0-9][A-Za-z0-9._-]*)"
    r"\s*(?P<operator>==|>=|<=|~=|!=|>|<)?\s*"
    r"(?P<version>[^;\s]+)?"
    r"(?:\s*;\s*(?P<marker>.+))?$"
)


def canonicalize(name: str) -> str:
    """PEP 503 name normalization, so `pdfminer.six` and `pdfminer-six` match."""
    return _CANONICALIZE_SEPARATORS.sub("-", name).lower()


@dataclass(frozen=True)
class LockedRequirement:
    name: str
    operator: str
    version: str
    marker: str
    hashes: tuple[str, ...]

    @property
    def canonical_name(self) -> str:
        return canonicalize(self.name)

    @property
    def is_exactly_pinned(self) -> bool:
        """Only `==` fixes a version; every other operator can still resolve."""
        return self.operator == "==" and bool(self.version)


class MalformedLockfile(ValueError):
    """The lock file cannot be parsed, so nothing about it can be trusted."""


def path() -> Path:
    return upstream.load_environment().lockfile


def _logical_lines(text: str) -> list[str]:
    """Join backslash continuations and drop comments and blank lines."""
    joined: str = text.replace("\\\n", " ")
    lines: list[str] = []
    for raw in joined.splitlines():
        without_comment: str = raw.split("#", 1)[0].strip()
        if without_comment:
            lines.append(without_comment)
    return lines


def _parse_line(line: str) -> LockedRequirement:
    tokens: list[str] = line.split()
    hashes: tuple[str, ...] = tuple(
        token[len(HASH_OPTION) :] for token in tokens if token.startswith(HASH_OPTION)
    )
    specifier: str = " ".join(token for token in tokens if not token.startswith(HASH_OPTION))
    matched = _REQUIREMENT.match(specifier)
    if matched is None:
        raise MalformedLockfile(f"cannot parse requirement: {specifier!r}")
    return LockedRequirement(
        name=matched.group("name"),
        operator=matched.group("operator") or "",
        version=matched.group("version") or "",
        marker=(matched.group("marker") or "").strip(),
        hashes=hashes,
    )


def load(lockfile: Path | None = None) -> list[LockedRequirement]:
    resolved: Path = lockfile if lockfile is not None else path()
    return [_parse_line(line) for line in _logical_lines(resolved.read_text())]


def find(requirements: list[LockedRequirement], name: str) -> LockedRequirement:
    wanted: str = canonicalize(name)
    for requirement in requirements:
        if requirement.canonical_name == wanted:
            return requirement
    raise KeyError(f"{name} is not locked")


def digest(lockfile: Path | None = None) -> str:
    """The lock file's own SHA-256, recorded in every golden artifact."""
    resolved: Path = lockfile if lockfile is not None else path()
    return hashlib.sha256(resolved.read_bytes()).hexdigest()
