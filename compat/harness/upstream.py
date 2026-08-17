"""The pinned upstream compatibility target (PARITY-001).

Every part of the harness reads the target from one place so a version bump
cannot be applied to the lock file, the golden data, and the parity report
independently and leave them disagreeing about what was compared.
"""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parents[2]
TARGET_FILE: Path = REPO_ROOT / "compat" / "upstream.toml"


@dataclass(frozen=True)
class Target:
    """The exact upstream release this repository is measured against."""

    project: str
    version: str
    tag: str
    commit: str
    repository: str


@dataclass(frozen=True)
class Environment:
    """Where the reference and candidate interpreters live."""

    python_version: str
    lockfile: Path
    reference_venv: Path
    candidate_venv: Path


def _read() -> dict[str, dict[str, str]]:
    with TARGET_FILE.open("rb") as handle:
        return tomllib.load(handle)


def load_target() -> Target:
    section: dict[str, str] = _read()["target"]
    return Target(
        project=section["project"],
        version=section["version"],
        tag=section["tag"],
        commit=section["commit"],
        repository=section["repository"],
    )


def load_environment() -> Environment:
    section: dict[str, str] = _read()["environment"]
    return Environment(
        python_version=section["python_version"],
        lockfile=REPO_ROOT / section["lockfile"],
        reference_venv=REPO_ROOT / section["reference_venv"],
        candidate_venv=REPO_ROOT / section["candidate_venv"],
    )
