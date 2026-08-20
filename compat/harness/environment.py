"""Reference and candidate environment isolation (PARITY-004).

Upstream Python `pdfplumber` and this project's binding are both imported as
`pdfplumber`. A runner that imports the wrong one compares an implementation
against itself and reports flawless parity — the most expensive kind of silent
failure this repository can have. These guards turn that into a loud error at
the point of import.

The two environments are kept in separate virtual environments rather than
separated by `sys.path` order, because path order is easy to disturb and hard to
notice once disturbed.
"""

from __future__ import annotations

from pathlib import Path
from types import ModuleType

from compat.harness import upstream

REPO_ROOT: Path = upstream.REPO_ROOT

_environment: upstream.Environment = upstream.load_environment()
REFERENCE_VENV: Path = _environment.reference_venv
CANDIDATE_VENV: Path = _environment.candidate_venv

# Upstream ships only pure Python; the candidate ships a pure-Python package
# containing a private compiled extension. Listed explicitly rather than read
# from `importlib.machinery` so the check gives the same answer regardless of
# which platform is running it.
NATIVE_SUFFIXES: tuple[str, ...] = (".so", ".pyd", ".dylib")


class EnvironmentMismatch(RuntimeError):
    """The imported `pdfplumber` is not the one this run requires."""


def _module_path(module: ModuleType) -> Path:
    location: str | None = getattr(module, "__file__", None)
    if not location:
        raise EnvironmentMismatch(
            f"{module.__name__} has no __file__, so its origin cannot be verified"
        )
    return Path(location)


def _is_native_extension(location: Path) -> bool:
    return any(location.name.endswith(suffix) for suffix in NATIVE_SUFFIXES)


def verify_reference(module: ModuleType, expected_root: Path | None = None) -> None:
    """Raise unless `module` is the pinned upstream Python `pdfplumber`."""
    target: upstream.Target = upstream.load_target()
    location: Path = _module_path(module)

    if _is_native_extension(location):
        raise EnvironmentMismatch(
            f"reference environment imported a compiled module at {location}; "
            f"expected pure-Python {target.project} {target.version}"
        )

    version: str | None = getattr(module, "__version__", None)
    if version is None:
        raise EnvironmentMismatch(
            f"reference environment imported {location} with no __version__; "
            f"expected {target.project} {target.version}"
        )
    if version != target.version:
        raise EnvironmentMismatch(
            f"reference environment has {target.project} {version}, "
            f"but the pinned compatibility target is {target.version}"
        )

    if expected_root is None:
        return
    # Resolved on both sides: on macOS a temporary or /tmp-based checkout reaches
    # the same directory under two different paths, and an unresolved comparison
    # would reject a perfectly correct environment.
    root: Path = expected_root.resolve()
    if root not in location.resolve().parents:
        raise EnvironmentMismatch(
            f"reference {target.project} was imported from {location}, "
            f"outside the reference environment at {root}"
        )


def verify_candidate(module: ModuleType, expected_root: Path | None = None) -> None:
    """Raise unless `module` is this project's mixed Python/native package."""
    location: Path = _module_path(module)
    native: ModuleType | None = getattr(module, "_native", None)
    if not isinstance(native, ModuleType):
        target: upstream.Target = upstream.load_target()
        raise EnvironmentMismatch(
            f"candidate environment imported {location} without "
            f"{target.project}._native; this is not the Rust candidate package"
        )

    native_location: Path = _module_path(native)
    expected_native_name = f"{module.__name__}._native"
    if native.__name__ != expected_native_name:
        raise EnvironmentMismatch(
            f"candidate native module is {native.__name__}, expected "
            f"{expected_native_name}"
        )
    if not _is_native_extension(native_location):
        raise EnvironmentMismatch(
            f"candidate {expected_native_name} at {native_location} is not a "
            "compiled extension"
        )

    if expected_root is None:
        return
    root: Path = expected_root.resolve()
    for role, module_location in (
        ("package", location),
        ("native extension", native_location),
    ):
        if root not in module_location.resolve().parents:
            raise EnvironmentMismatch(
                f"candidate {role} was imported from {module_location}, "
                f"outside the candidate environment at {root}"
            )
