#!/usr/bin/env python3
"""Build and verify the isolated installed-candidate environment (PARITY-004)."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[1]
UPSTREAM_CONFIG = REPO_ROOT / "compat" / "upstream.toml"
MATURIN_VERSION = "1.14.1"


def resolve_candidate_venv(repository: Path, configured: str) -> Path:
    """Return a deletion-safe candidate venv path directly under the repository."""
    relative = Path(configured)
    if (
        relative.is_absolute()
        or len(relative.parts) != 1
        or relative.name in {"", ".", ".."}
        or not relative.name.startswith(".venv-")
    ):
        raise ValueError(
            "candidate_venv must be a named .venv-* directory directly under "
            f"the repository, got {configured!r}"
        )
    candidate = (repository / relative).resolve()
    if candidate.parent != repository.resolve():
        raise ValueError(f"unsafe candidate environment path: {candidate}")
    return candidate


def require_single_wheel(wheels: list[Path]) -> Path:
    """Return the sole existing wheel or reject ambiguous/stale build input."""
    if len(wheels) != 1:
        raise ValueError(f"expected exactly one candidate wheel, found {len(wheels)}")
    wheel = wheels[0].resolve()
    if not wheel.is_file() or wheel.suffix != ".whl":
        raise ValueError(f"candidate wheel does not exist or is not a .whl: {wheel}")
    return wheel


def load_settings() -> tuple[str, Path]:
    with UPSTREAM_CONFIG.open("rb") as handle:
        config = tomllib.load(handle)["environment"]
    python_version = str(config["python_version"])
    candidate_venv = resolve_candidate_venv(
        REPO_ROOT, str(config["candidate_venv"])
    )
    return python_version, candidate_venv


def select_python(explicit: str | None, pinned_version: str) -> str:
    requested = explicit or os.environ.get("PDFPLUMBER_RS_CANDIDATE_PYTHON")
    candidates = [requested] if requested else [f"python{pinned_version}", "python3"]
    interpreter = next(
        (candidate for candidate in candidates if candidate and shutil.which(candidate)),
        None,
    )
    if interpreter is None:
        raise ValueError(
            f"no Python {pinned_version} candidate interpreter is available; "
            "pass --python or set PDFPLUMBER_RS_CANDIDATE_PYTHON"
        )
    completed = subprocess.run(
        [
            interpreter,
            "-c",
            "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    actual = completed.stdout.strip()
    if actual != pinned_version:
        raise ValueError(
            f"candidate interpreter {interpreter!r} is Python {actual}, "
            f"but compatibility requires Python {pinned_version}"
        )
    return interpreter


def require_maturin() -> str:
    executable = shutil.which("maturin")
    if executable is None:
        raise ValueError(
            f"maturin {MATURIN_VERSION} is required to build a candidate wheel"
        )
    completed = subprocess.run(
        [executable, "--version"], check=True, capture_output=True, text=True
    )
    actual = completed.stdout.strip()
    if actual != f"maturin {MATURIN_VERSION}":
        raise ValueError(
            f"candidate build requires maturin {MATURIN_VERSION}, got {actual!r}"
        )
    return executable


def build_wheel(interpreter: str, output: Path) -> Path:
    maturin = require_maturin()
    subprocess.run(
        [
            maturin,
            "build",
            "--manifest-path",
            str(REPO_ROOT / "crates" / "pdfplumber-py" / "Cargo.toml"),
            "--interpreter",
            interpreter,
            "--release",
            "--out",
            str(output),
        ],
        cwd=REPO_ROOT,
        check=True,
    )
    return require_single_wheel(sorted(output.glob("*.whl")))


def environment_python(venv: Path) -> Path:
    unix = venv / "bin" / "python"
    if unix.is_file():
        return unix
    return venv / "Scripts" / "python.exe"


def install_candidate(interpreter: str, venv: Path, wheel: Path) -> Path:
    if venv.exists():
        shutil.rmtree(venv)
    subprocess.run([interpreter, "-m", "venv", str(venv)], check=True)
    candidate_python = environment_python(venv)
    subprocess.run(
        [
            str(candidate_python),
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--no-deps",
            str(wheel),
        ],
        cwd=REPO_ROOT,
        check=True,
    )
    subprocess.run(
        [
            str(candidate_python),
            str(REPO_ROOT / "scripts" / "verify_compat_env.py"),
            "--candidate",
            "--expect-root",
            str(venv),
        ],
        cwd=REPO_ROOT,
        check=True,
    )
    return candidate_python


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--python",
        help="Python interpreter; must match compat/upstream.toml",
    )
    parser.add_argument(
        "--wheel",
        type=Path,
        help="install this prebuilt local wheel instead of invoking maturin",
    )
    arguments = parser.parse_args(argv)

    pinned_version, candidate_venv = load_settings()
    interpreter = select_python(arguments.python, pinned_version)
    if arguments.wheel is not None:
        wheel = require_single_wheel([arguments.wheel])
        candidate_python = install_candidate(
            interpreter, candidate_venv, wheel
        )
    else:
        with TemporaryDirectory(prefix="pdfplumber-candidate-wheel-") as directory:
            wheel = build_wheel(interpreter, Path(directory))
            candidate_python = install_candidate(
                interpreter, candidate_venv, wheel
            )

    print(f"Candidate environment ready: {candidate_python}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
