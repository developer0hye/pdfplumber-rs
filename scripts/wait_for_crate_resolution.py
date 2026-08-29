#!/usr/bin/env python3
"""Wait until Cargo can resolve one exact crates.io package version."""

from __future__ import annotations

import argparse
import math
import re
import subprocess
import sys
import time
from collections.abc import Callable, Sequence
from typing import NamedTuple

DEFAULT_TIMEOUT_SECONDS = 300.0
DEFAULT_INITIAL_DELAY_SECONDS = 5.0
DEFAULT_MAXIMUM_DELAY_SECONDS = 30.0
DEFAULT_PROBE_TIMEOUT_SECONDS = 30.0
SEMVER_PATTERN = re.compile(
    r"(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


class CrateResolutionError(RuntimeError):
    """A crates.io package did not become resolvable within policy."""


class ResolutionResult(NamedTuple):
    """Successful registry-resolution evidence."""

    package: str
    version: str
    attempts: int
    elapsed_seconds: float


Runner = Callable[[tuple[str, ...], float], subprocess.CompletedProcess[str]]
Clock = Callable[[], float]
Sleeper = Callable[[float], None]
Emitter = Callable[[str], None]


def version_from_release_tag(release_tag: str) -> str:
    """Return the exact SemVer encoded by a release tag."""

    if not release_tag.startswith("v") or not SEMVER_PATTERN.fullmatch(release_tag[1:]):
        raise CrateResolutionError(
            f"release tag must be v-prefixed SemVer, got {release_tag!r}"
        )
    return release_tag[1:]


def run_cargo_info(
    command: tuple[str, ...],
    timeout_seconds: float,
) -> subprocess.CompletedProcess[str]:
    """Run one time-bounded Cargo registry probe without exposing its output."""

    return subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
    )


def validate_policy(
    package: str,
    version: str,
    timeout_seconds: float,
    initial_delay_seconds: float,
    maximum_delay_seconds: float,
    probe_timeout_seconds: float,
) -> None:
    """Reject ambiguous identifiers and unbounded or invalid timing policy."""

    if (
        not package
        or package.strip() != package
        or any(character.isspace() for character in package)
    ):
        raise CrateResolutionError("package must be a non-empty Cargo package name")
    if not SEMVER_PATTERN.fullmatch(version):
        raise CrateResolutionError(f"version must be SemVer, got {version!r}")
    for label, value in (
        ("timeout_seconds", timeout_seconds),
        ("initial_delay_seconds", initial_delay_seconds),
        ("maximum_delay_seconds", maximum_delay_seconds),
        ("probe_timeout_seconds", probe_timeout_seconds),
    ):
        if not math.isfinite(value) or value <= 0:
            raise CrateResolutionError(f"{label} must be finite and greater than zero")
    if maximum_delay_seconds < initial_delay_seconds:
        raise CrateResolutionError(
            "maximum_delay_seconds must be at least initial_delay_seconds"
        )


def wait_until_resolvable(
    package: str,
    version: str,
    *,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    initial_delay_seconds: float = DEFAULT_INITIAL_DELAY_SECONDS,
    maximum_delay_seconds: float = DEFAULT_MAXIMUM_DELAY_SECONDS,
    probe_timeout_seconds: float = DEFAULT_PROBE_TIMEOUT_SECONDS,
    runner: Runner = run_cargo_info,
    monotonic: Clock = time.monotonic,
    sleeper: Sleeper = time.sleep,
    emit: Emitter = print,
) -> ResolutionResult:
    """Poll Cargo with capped backoff until an exact registry version resolves."""

    validate_policy(
        package,
        version,
        timeout_seconds,
        initial_delay_seconds,
        maximum_delay_seconds,
        probe_timeout_seconds,
    )
    package_spec = f"{package}@{version}"
    command = (
        "cargo",
        "info",
        package_spec,
        "--registry",
        "crates-io",
        "--color",
        "never",
    )
    start_seconds = monotonic()
    deadline_seconds = start_seconds + timeout_seconds
    delay_seconds = initial_delay_seconds
    attempts = 0

    while True:
        now_seconds = monotonic()
        if attempts > 0 and now_seconds >= deadline_seconds:
            elapsed_seconds = now_seconds - start_seconds
            emit(
                "ERROR crate_registry_probe "
                f"package={package} version={version} attempts={attempts} "
                f"outcome=timeout elapsed_seconds={elapsed_seconds:.3f}"
            )
            raise CrateResolutionError(
                f"timed out waiting for {package_spec} after {attempts} attempts "
                f"and {elapsed_seconds:.3f}s"
            )

        attempts += 1
        remaining_seconds = deadline_seconds - now_seconds
        current_probe_timeout = min(probe_timeout_seconds, remaining_seconds)
        emit(
            "INFO crate_registry_probe "
            f"package={package} version={version} attempt={attempts} "
            f"outcome=probing elapsed_seconds={now_seconds - start_seconds:.3f}"
        )

        try:
            result = runner(command, current_probe_timeout)
            is_resolvable = result.returncode == 0
        except FileNotFoundError as error:
            raise CrateResolutionError(
                "cargo executable is unavailable; install Cargo before polling crates.io"
            ) from error
        except subprocess.TimeoutExpired:
            is_resolvable = False
        except OSError:
            is_resolvable = False

        now_seconds = monotonic()
        elapsed_seconds = now_seconds - start_seconds
        if is_resolvable:
            emit(
                "INFO crate_registry_probe "
                f"package={package} version={version} attempts={attempts} "
                f"outcome=resolved elapsed_seconds={elapsed_seconds:.3f}"
            )
            return ResolutionResult(
                package=package,
                version=version,
                attempts=attempts,
                elapsed_seconds=elapsed_seconds,
            )

        remaining_seconds = deadline_seconds - now_seconds
        if remaining_seconds <= 0:
            continue
        current_delay = min(delay_seconds, remaining_seconds)
        emit(
            "WARN crate_registry_probe "
            f"package={package} version={version} attempt={attempts} "
            f"outcome=retry next_delay_seconds={current_delay:.3f} "
            f"elapsed_seconds={elapsed_seconds:.3f}"
        )
        sleeper(current_delay)
        delay_seconds = min(delay_seconds * 2, maximum_delay_seconds)


def positive_seconds(value: str) -> float:
    """Parse a positive duration for argparse."""

    try:
        seconds = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not math.isfinite(seconds) or seconds <= 0:
        raise argparse.ArgumentTypeError("must be finite and greater than zero")
    return seconds


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="wait until Cargo resolves an exact crates.io package version"
    )
    parser.add_argument("package", help="crates.io package name")
    version_group = parser.add_mutually_exclusive_group(required=True)
    version_group.add_argument("--version", help="exact expected SemVer")
    version_group.add_argument(
        "--release-tag",
        help="v-prefixed release tag that supplies the exact expected version",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=positive_seconds,
        default=DEFAULT_TIMEOUT_SECONDS,
    )
    parser.add_argument(
        "--initial-delay-seconds",
        type=positive_seconds,
        default=DEFAULT_INITIAL_DELAY_SECONDS,
    )
    parser.add_argument(
        "--maximum-delay-seconds",
        type=positive_seconds,
        default=DEFAULT_MAXIMUM_DELAY_SECONDS,
    )
    parser.add_argument(
        "--probe-timeout-seconds",
        type=positive_seconds,
        default=DEFAULT_PROBE_TIMEOUT_SECONDS,
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        version = (
            args.version
            if args.version is not None
            else version_from_release_tag(args.release_tag)
        )
        wait_until_resolvable(
            args.package,
            version,
            timeout_seconds=args.timeout_seconds,
            initial_delay_seconds=args.initial_delay_seconds,
            maximum_delay_seconds=args.maximum_delay_seconds,
            probe_timeout_seconds=args.probe_timeout_seconds,
        )
    except CrateResolutionError as error:
        print(
            f"ERROR crate_registry_gate outcome=failed reason={error}", file=sys.stderr
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
