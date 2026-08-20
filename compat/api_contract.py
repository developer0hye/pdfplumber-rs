#!/usr/bin/env python3
"""Generate or verify executable call contracts for pinned pdfplumber."""

from __future__ import annotations

import argparse
import difflib
import json
import os
import sys
from pathlib import Path

import pdfplumber

SCRIPT_DIR: Path = Path(os.path.abspath(__file__)).parent
REPO_ROOT: Path = SCRIPT_DIR.parent
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import call_contract, environment  # noqa: E402


def render(contract: dict[str, object]) -> str:
    return json.dumps(contract, indent=2, ensure_ascii=False, sort_keys=True) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--write-reference",
        action="store_true",
        help="replace the committed contract from the pinned reference package",
    )
    mode.add_argument(
        "--reference",
        action="store_true",
        help="require and verify the pinned reference package",
    )
    mode.add_argument(
        "--candidate",
        action="store_true",
        help="require a candidate package and compare its case outcomes",
    )
    return parser.parse_args()


def main() -> int:
    args: argparse.Namespace = parse_args()
    reference_mode: bool
    if args.write_reference or args.reference:
        reference_mode = True
    elif args.candidate:
        reference_mode = False
    else:
        reference_mode = _is_reference_import()

    try:
        if reference_mode:
            environment.verify_reference(
                pdfplumber, expected_root=environment.REFERENCE_VENV
            )
        else:
            environment.verify_candidate(
                pdfplumber, expected_root=environment.CANDIDATE_VENV
            )
    except environment.EnvironmentMismatch as mismatch:
        mode_name: str = "reference" if reference_mode else "candidate"
        print(f"refusing to run {mode_name} call contracts: {mismatch}", file=sys.stderr)
        if reference_mode:
            print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        return 1

    output_path: Path = call_contract.contract_path()
    generated: str = render(call_contract.build(pdfplumber))

    if args.write_reference:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(generated, encoding="utf-8")
        print(f"Wrote call contract: {output_path.relative_to(REPO_ROOT)}")
        return 0

    if not output_path.is_file():
        print(f"Call contract is missing: {output_path.relative_to(REPO_ROOT)}")
        return 1

    committed: str = output_path.read_text(encoding="utf-8")
    if reference_mode:
        expected: str = committed
        actual: str = generated
        success_message: str = (
            f"Call contract is current: {output_path.relative_to(REPO_ROOT)}"
        )
        stale_message: str = (
            f"Call contract is stale: {output_path.relative_to(REPO_ROOT)}"
        )
    else:
        expected = render_cases(committed)
        actual = render_cases(generated)
        success_message = "Candidate call outcomes match the pinned reference"
        stale_message = "Candidate call outcomes differ from the pinned reference"

    if expected == actual:
        print(success_message)
        return 0

    print(stale_message)
    diff: list[str] = list(
        difflib.unified_diff(
            expected.splitlines(),
            actual.splitlines(),
            fromfile="pinned-reference",
            tofile="current-run",
            lineterm="",
        )
    )
    for line in diff[:200]:
        print(line)
    if len(diff) > 200:
        print(f"... {len(diff) - 200} additional diff lines omitted")
    return 1


def render_cases(serialized_contract: str) -> str:
    contract: dict[str, object] = json.loads(serialized_contract)
    return json.dumps(
        contract["cases"], indent=2, ensure_ascii=False, sort_keys=True
    ) + "\n"


def _is_reference_import() -> bool:
    try:
        environment.verify_reference(
            pdfplumber, expected_root=environment.REFERENCE_VENV
        )
    except environment.EnvironmentMismatch:
        return False
    return True


if __name__ == "__main__":
    raise SystemExit(main())
