#!/usr/bin/env python3
"""Compare pdfplumber-rs output against Python pdfplumber, fixture by fixture.

Reports, per page of every PDF, how closely the Rust CLI matches Python
pdfplumber on characters, page text, layout text, simple text, text lines,
words, search results, lattice tables, annotations, hyperlinks, and structure
trees. Character and word ratios are position-sensitive: matching an object
elsewhere in the sequence does not hide an ordering difference. Object
dictionary structure is compared recursively without projecting away upstream
keys, value types, nested containers, or explicit null placement.
Use it to pick the next parity gap to close and to confirm a change moved the
numbers in the right direction.

Must run inside the pinned reference environment — see scripts/setup_golden_venv.sh.
The interpreter is checked on startup, because a report generated against the
wrong pdfplumber compares an implementation with itself and always looks perfect
(PARITY-004).

    .venv-reference/bin/python scripts/parity_report.py
    .venv-reference/bin/python scripts/parity_report.py --repo ../pdfplumber-rs-some-worktree
    .venv-reference/bin/python scripts/parity_report.py --json report.json
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, REPO_ROOT)

from compat.harness import approved_deltas, environment, upstream  # noqa: E402

FIXTURE_DIRS = [
    "tests/fixtures/generated",
    "tests/fixtures/downloaded",
    "crates/pdfplumber/tests/fixtures/pdfs",
]

# Coordinates are compared at this tolerance, in points. Anything larger is a
# real disagreement about where the object sits, not float noise.
COORD_TOLERANCE = 0.05
FIXTURE_COLUMN_WIDTH = 60
SEARCH_PATTERN = r"\S+"
UNSUPPORTED_API_MARKER = "unsupported"
PARITY_APIS = (
    "page_text",
    "layout_text",
    "simple_text",
    "text_lines",
    "words",
    "search",
    "tables",
    "annotations",
    "hyperlinks",
    "structure_tree",
)
APPROVED_DELTAS_PATH = os.path.join(REPO_ROOT, "compat", "approved_deltas.toml")


def find_fixtures(fixture_root: str) -> list[str]:
    paths = []
    for rel_dir in FIXTURE_DIRS:
        directory = os.path.join(fixture_root, rel_dir)
        if not os.path.isdir(directory):
            continue
        for name in sorted(os.listdir(directory)):
            if name.endswith(".pdf"):
                paths.append(os.path.join(directory, name))
    return paths


def fixture_id(path: str, fixture_root: str) -> str:
    """Return a stable corpus-relative ID without basename collisions."""
    return os.path.relpath(path, fixture_root).replace(os.sep, "/")


def run_cli_output(repo: str, args: list[str]) -> str:
    """Run the pdfplumber CLI in `repo` and return its standard output."""
    result = subprocess.run(
        ["cargo", "run", "-q", "-p", "pdfplumber-cli", "--", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip()[-500:])
    return result.stdout


def run_cli(repo: str, args: list[str]) -> Any:
    """Run the pdfplumber CLI in `repo` and parse one JSON value."""
    output = run_cli_output(repo, args)
    return json.loads(output) if output.strip() else []


def run_cli_json_lines(repo: str, args: list[str]) -> list[Any]:
    """Run a CLI command that emits one JSON value per output line."""
    return [json.loads(line) for line in run_cli_output(repo, args).splitlines() if line.strip()]


def close(a: float, b: float) -> bool:
    return abs(a - b) <= COORD_TOLERANCE


def key_of(obj: dict, with_box: bool = True) -> tuple:
    """A comparable identity for an object: its text and, optionally, its box."""
    if not with_box:
        return (obj["text"],)
    return (
        obj["text"],
        round(obj["x0"] / COORD_TOLERANCE),
        round(obj["top"] / COORD_TOLERANCE),
        round(obj["x1"] / COORD_TOLERANCE),
        round(obj["bottom"] / COORD_TOLERANCE),
    )


def compare_object_sequence(expected: list[dict], actual: list[dict], with_box: bool) -> dict:
    """Compare object identities at the same sequence positions."""
    expected_keys = [key_of(obj, with_box) for obj in expected]
    actual_keys = [key_of(obj, with_box) for obj in actual]
    total = max(len(expected), len(actual))
    matched = sum(left == right for left, right in zip(expected_keys, actual_keys))
    return {
        "matched": matched,
        "total": total,
        "ratio": 1.0 if total == 0 else matched / total,
        "order_equal": expected_keys == actual_keys,
    }


def type_name(value: Any) -> str:
    """Return a stable, fully qualified runtime type name."""
    value_type = type(value)
    return f"{value_type.__module__}.{value_type.__qualname__}"


def structural_signature(value: Any) -> tuple:
    """Describe keys, runtime types, nesting, and null positions, not values."""
    if value is None:
        return ("none",)
    if isinstance(value, dict):
        items = [
            ((type_name(key), repr(key)), structural_signature(nested))
            for key, nested in value.items()
        ]
        return ("dict", type_name(value), tuple(sorted(items, key=lambda item: item[0])))
    if isinstance(value, list):
        return (
            "list",
            type_name(value),
            tuple(structural_signature(item) for item in value),
        )
    if isinstance(value, tuple):
        return (
            "tuple",
            type_name(value),
            tuple(structural_signature(item) for item in value),
        )
    return ("scalar", type_name(value))


def compare_dictionary_sequence(expected: list[dict], actual: list[dict]) -> dict:
    """Compare complete dictionary structure at matching sequence positions."""
    expected_signatures = [structural_signature(obj) for obj in expected]
    actual_signatures = [structural_signature(obj) for obj in actual]
    total = max(len(expected), len(actual))
    matched = sum(
        left == right for left, right in zip(expected_signatures, actual_signatures)
    )
    return {
        "matched": matched,
        "total": total,
        "ratio": 1.0 if total == 0 else matched / total,
        "structure_equal": expected_signatures == actual_signatures,
    }


def compare_chars(expected: list[dict], actual: list[dict]) -> dict:
    """How far the characters agree, by text alone and by text plus position."""
    text = compare_object_sequence(expected, actual, with_box=False)
    boxes = compare_object_sequence(expected, actual, with_box=True)
    dictionaries = compare_dictionary_sequence(expected, actual)
    return {
        "count_expected": len(expected),
        "count_actual": len(actual),
        "text_matched": text["matched"],
        "text_ratio": text["ratio"],
        "text_order_equal": text["order_equal"],
        "box_matched": boxes["matched"],
        "box_ratio": boxes["ratio"],
        "box_order_equal": boxes["order_equal"],
        "dictionary": dictionaries,
    }


def compare_words(expected: list[dict], actual: list[dict]) -> dict:
    comparison = compare_object_sequence(expected, actual, with_box=True)
    return {
        "count_expected": len(expected),
        "count_actual": len(actual),
        "equal": expected == actual,
        **comparison,
        "dictionary": compare_dictionary_sequence(expected, actual),
    }


def compare_text(expected: str, actual: str) -> dict:
    return {"equal": expected == actual, "len_expected": len(expected), "len_actual": len(actual)}


def compare_tables(expected: list, actual: list) -> dict:
    expected_cells = sum(len(row) for table in expected for row in table)
    actual_cells = sum(len(row) for table in actual for row in table)
    matching = 0
    for want_table, got_table in zip(expected, actual):
        for want_row, got_row in zip(want_table, got_table):
            for want_cell, got_cell in zip(want_row, got_row):
                if want_cell == got_cell:
                    matching += 1
    total_cells = max(expected_cells, actual_cells)
    return {
        "tables_expected": len(expected),
        "tables_actual": len(actual),
        "cells_expected": expected_cells,
        "cells_actual": actual_cells,
        "equal": expected == actual,
        # Agreeing that a page holds no tables is a match, not a miss.
        "cell_ratio": 1.0 if total_cells == 0 else matching / total_cells,
        "structure_equal": structural_signature(expected) == structural_signature(actual),
    }


def unsupported_api(task_id: str) -> dict:
    """Represent an unavailable candidate API without omitting it."""
    return {"status": UNSUPPORTED_API_MARKER, "task_id": task_id}


def compare_api_value(expected: Any, actual: Any) -> dict:
    """Compare arbitrary API values while preserving unsupported candidates."""
    if (
        isinstance(actual, dict)
        and actual.get("status") == UNSUPPORTED_API_MARKER
        and isinstance(actual.get("task_id"), str)
    ):
        return {
            "status": "unsupported_in_rust",
            "task_id": actual["task_id"],
            "equal": False,
            "structure_equal": False,
        }

    result = {
        "status": "compared",
        "equal": expected == actual,
        "structure_equal": structural_signature(expected)
        == structural_signature(actual),
    }
    if isinstance(expected, list) and isinstance(actual, list):
        result["count_expected"] = len(expected)
        result["count_actual"] = len(actual)
    return result


def unsupported_api_names(page_result: dict) -> list[str]:
    """Return candidate APIs that the page comparison could not exercise."""
    return [
        name
        for name in PARITY_APIS
        if page_result[name].get("status") == "unsupported_in_rust"
    ]


def python_page(page: Any, page_number: int) -> dict:
    return {
        "page_number": page_number,
        "chars": [dict(char) for char in page.chars],
        "words": [dict(word) for word in page.extract_words()],
        "page_text": page.extract_text() or "",
        "layout_text": page.extract_text(layout=True) or "",
        "simple_text": page.extract_text_simple() or "",
        "text_lines": [dict(line) for line in page.extract_text_lines()],
        "search": [dict(match) for match in page.search(SEARCH_PATTERN)],
        "tables": page.extract_tables(),
        "annotations": [dict(annotation) for annotation in page.annots],
        "hyperlinks": [dict(link) for link in page.hyperlinks],
        "structure_tree": [dict(element) for element in page.structure_tree],
    }


def python_side(path: str, reference_package: Any = None) -> dict:
    if reference_package is None:
        # Keep structural unit-test discovery independent of the pinned venv.
        # The executable path verifies this lazily imported package before it
        # passes it here; direct callers run under .venv-reference as well.
        import pdfplumber as reference_package

    with reference_package.open(path) as pdf:
        pages = [python_page(page, number) for number, page in enumerate(pdf.pages, start=1)]
        return {"page_count": len(pages), "pages": pages}


def rust_page_number(item: Any, label: str, page_count: int) -> int:
    if not isinstance(item, dict):
        raise RuntimeError(f"Rust {label} output item is not an object: {item!r}")
    page_number = item.get("page")
    if isinstance(page_number, bool) or not isinstance(page_number, int):
        raise RuntimeError(f"Rust {label} output has an invalid page number: {page_number!r}")
    if not 1 <= page_number <= page_count:
        raise RuntimeError(
            f"Rust {label} output page {page_number} is outside document range 1-{page_count}"
        )
    return page_number


def rust_side(repo: str, path: str) -> dict:
    info = run_cli(repo, ["info", path, "--format", "json"])
    page_count = info.get("pages") if isinstance(info, dict) else None
    if isinstance(page_count, bool) or not isinstance(page_count, int) or page_count < 0:
        raise RuntimeError(f"Rust info output has an invalid page count: {page_count!r}")

    pages = [
        {
            "page_number": page_number,
            "chars": [],
            "words": [],
            "page_text": None,
            "layout_text": None,
            "simple_text": unsupported_api("TEXT-EXTRA-001"),
            "text_lines": None,
            "search": [],
            "tables": [],
            "annotations": [],
            "hyperlinks": [],
            "structure_tree": None,
        }
        for page_number in range(1, page_count + 1)
    ]

    json_args = ["--format", "json"]
    objects = {
        "chars": run_cli(repo, ["chars", path, *json_args]),
        "words": run_cli(repo, ["words", path, *json_args]),
        "search": run_cli(repo, ["search", path, SEARCH_PATTERN, *json_args]),
        "annotations": run_cli(repo, ["annots", path, *json_args]),
        "hyperlinks": run_cli(repo, ["links", path, *json_args]),
    }
    for label, items in objects.items():
        if not isinstance(items, list):
            raise RuntimeError(f"Rust {label} output is not a list")
        for item in items:
            page_number = rust_page_number(item, label, page_count)
            pages[page_number - 1][label].append(item)

    text_commands = {
        "page_text": ["text", path, *json_args],
        "layout_text": ["text", path, *json_args, "--layout"],
    }
    for label, command in text_commands.items():
        seen_pages = set()
        for item in run_cli_json_lines(repo, command):
            page_number = rust_page_number(item, label, page_count)
            if page_number in seen_pages:
                raise RuntimeError(f"Rust {label} output repeats page {page_number}")
            if not isinstance(item.get("text"), str):
                raise RuntimeError(
                    f"Rust {label} output for page {page_number} is not a string"
                )
            pages[page_number - 1][label] = item["text"]
            seen_pages.add(page_number)
        missing_pages = sorted(set(range(1, page_count + 1)) - seen_pages)
        if missing_pages:
            raise RuntimeError(f"Rust {label} output omits pages: {missing_pages}")

    snapshots = run_cli(repo, ["compat-snapshot", path])
    if not isinstance(snapshots, list):
        raise RuntimeError("Rust compatibility snapshot output is not a list")
    seen_snapshot_pages = set()
    for snapshot in snapshots:
        page_number = rust_page_number(snapshot, "compatibility snapshot", page_count)
        if page_number in seen_snapshot_pages:
            raise RuntimeError(f"Rust compatibility snapshot repeats page {page_number}")
        for field in ("text_lines", "structure_tree"):
            if not isinstance(snapshot.get(field), list):
                raise RuntimeError(
                    f"Rust compatibility snapshot {field} for page {page_number} is not a list"
                )
            pages[page_number - 1][field] = snapshot[field]
        seen_snapshot_pages.add(page_number)
    missing_snapshot_pages = sorted(
        set(range(1, page_count + 1)) - seen_snapshot_pages
    )
    if missing_snapshot_pages:
        raise RuntimeError(
            f"Rust compatibility snapshot omits pages: {missing_snapshot_pages}"
        )

    tables = run_cli(repo, ["tables", path, *json_args])
    if not isinstance(tables, list):
        raise RuntimeError("Rust tables output is not a list")
    for table in tables:
        page_number = rust_page_number(table, "tables", page_count)
        if "rows" not in table:
            raise RuntimeError(f"Rust table output for page {page_number} has no rows")
        pages[page_number - 1]["tables"].append(table["rows"])

    return {"page_count": page_count, "pages": pages}


def pages_by_number(document: dict, side: str) -> dict[int, dict]:
    pages = document.get("pages")
    page_count = document.get("page_count")
    if (
        not isinstance(pages, list)
        or isinstance(page_count, bool)
        or not isinstance(page_count, int)
        or page_count < 0
    ):
        raise ValueError(f"{side} document has an invalid page collection")

    indexed = {}
    for page in pages:
        page_number = page.get("page_number") if isinstance(page, dict) else None
        if isinstance(page_number, bool) or not isinstance(page_number, int):
            raise ValueError(f"{side} document has a page without an integer page_number")
        if page_number in indexed:
            raise ValueError(f"{side} document repeats page {page_number}")
        indexed[page_number] = page
    if len(indexed) != page_count:
        raise ValueError(
            f"{side} document declares {page_count} pages but supplies {len(indexed)}"
        )
    return indexed


def compare_documents(expected: dict, actual: dict) -> dict:
    expected_pages = pages_by_number(expected, "Python")
    actual_pages = pages_by_number(actual, "Rust")
    page_results = []
    for page_number in sorted(expected_pages.keys() | actual_pages.keys()):
        if page_number not in actual_pages:
            page_results.append({"page_number": page_number, "status": "missing_in_rust"})
            continue
        if page_number not in expected_pages:
            page_results.append({"page_number": page_number, "status": "missing_in_python"})
            continue
        python_page_result = expected_pages[page_number]
        rust_page_result = actual_pages[page_number]
        page_results.append(
            {
                "page_number": page_number,
                "status": "compared",
                "chars": compare_chars(python_page_result["chars"], rust_page_result["chars"]),
                "words": compare_words(python_page_result["words"], rust_page_result["words"]),
                "page_text": compare_text(
                    python_page_result["page_text"], rust_page_result["page_text"]
                ),
                "layout_text": compare_text(
                    python_page_result["layout_text"], rust_page_result["layout_text"]
                ),
                "simple_text": compare_api_value(
                    python_page_result["simple_text"], rust_page_result["simple_text"]
                ),
                "text_lines": compare_api_value(
                    python_page_result["text_lines"], rust_page_result["text_lines"]
                ),
                "search": compare_api_value(
                    python_page_result["search"], rust_page_result["search"]
                ),
                "tables": compare_tables(
                    python_page_result["tables"], rust_page_result["tables"]
                ),
                "annotations": compare_api_value(
                    python_page_result["annotations"], rust_page_result["annotations"]
                ),
                "hyperlinks": compare_api_value(
                    python_page_result["hyperlinks"], rust_page_result["hyperlinks"]
                ),
                "structure_tree": compare_api_value(
                    python_page_result["structure_tree"], rust_page_result["structure_tree"]
                ),
            }
        )

    return {
        "page_count_expected": expected["page_count"],
        "page_count_actual": actual["page_count"],
        "page_count_equal": expected["page_count"] == actual["page_count"],
        "pages": page_results,
    }


def observed_document_deltas(
    fixture: str,
    expected: dict,
    actual: dict,
    comparison: dict,
) -> list[approved_deltas.ObservedDelta]:
    """Return exact result identities for differences the report observes."""
    expected_pages = pages_by_number(expected, "Python")
    actual_pages = pages_by_number(actual, "Rust")
    observations: list[approved_deltas.ObservedDelta] = []
    for page in comparison["pages"]:
        page_number = page["page_number"]
        if page["status"] != "compared":
            continue
        expected_page = expected_pages[page_number]
        actual_page = actual_pages[page_number]
        differing_apis: list[str] = []
        chars = page["chars"]
        if not (
            chars["count_expected"] == chars["count_actual"]
            and chars["text_order_equal"]
            and chars["box_order_equal"]
            and chars["dictionary"]["structure_equal"]
        ):
            differing_apis.append("chars")
        if not page["words"]["equal"]:
            differing_apis.append("words")
        for api in (
            "page_text",
            "layout_text",
            "simple_text",
            "text_lines",
            "search",
            "tables",
            "annotations",
            "hyperlinks",
            "structure_tree",
        ):
            result = page[api]
            if result.get("status", "compared") == "compared" and not result["equal"]:
                differing_apis.append(api)
        for api in differing_apis:
            observations.append(
                approved_deltas.ObservedDelta(
                    fixture=fixture,
                    page=page_number,
                    api=api,
                    upstream_sha256=approved_deltas.value_digest(expected_page[api]),
                    rust_sha256=approved_deltas.value_digest(actual_page[api]),
                )
            )
    return observations


def main(reference_package: Any = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=REPO_ROOT, help="worktree to test (default: this repo)")
    parser.add_argument("--fixtures", default=REPO_ROOT, help="repo holding tests/fixtures")
    parser.add_argument("--json", help="also write the full report here")
    parser.add_argument("--only", help="substring filter on the fixture filename")
    parser.add_argument(
        "--approved-deltas",
        default=APPROVED_DELTAS_PATH,
        help="exact intentional-difference registry",
    )
    args = parser.parse_args()

    try:
        registry = approved_deltas.load_registry(Path(args.approved_deltas))
        target = upstream.load_target()
        approved_deltas.validate_target(registry, target.version, target.commit)
    except approved_deltas.DeltaRegistryError as mismatch:
        print(f"refusing to report parity: {mismatch}", file=sys.stderr)
        return 1

    try:
        if reference_package is None:
            import pdfplumber as reference_package

        environment.verify_reference(reference_package)
    except environment.EnvironmentMismatch as mismatch:
        print(f"refusing to report parity: {mismatch}", file=sys.stderr)
        print("Run: bash scripts/setup_golden_venv.sh", file=sys.stderr)
        return 1

    report = {}
    compared_pages = []
    observed_deltas: list[approved_deltas.ObservedDelta] = []
    had_failure = False
    print(
        f"{'fixture':<{FIXTURE_COLUMN_WIDTH}} {'page':>4} "
        f"{'chars':>14} {'words':>8} {'text':>6} {'tables':>8} {'unsupported':>11}"
    )
    print("-" * (FIXTURE_COLUMN_WIDTH + 57))

    for path in find_fixtures(args.fixtures):
        name = fixture_id(path, args.fixtures)
        if args.only and args.only not in name:
            continue
        try:
            expected = python_side(path, reference_package)
        except Exception as exc:  # noqa: BLE001 - report and continue
            print(f"{name:<{FIXTURE_COLUMN_WIDTH}} python failed: {exc}")
            report[name] = {"status": "python_failed", "error": str(exc)}
            had_failure = True
            continue
        try:
            actual = rust_side(args.repo, path)
        except Exception as exc:  # noqa: BLE001 - report and continue
            print(f"{name:<{FIXTURE_COLUMN_WIDTH}} rust failed: {exc}")
            report[name] = {"status": "rust_failed", "error": str(exc)}
            had_failure = True
            continue

        entry = compare_documents(expected, actual)
        report[name] = entry
        try:
            observed_deltas.extend(
                observed_document_deltas(name, expected, actual, entry)
            )
        except approved_deltas.DeltaRegistryError as error:
            print(f"{name:<{FIXTURE_COLUMN_WIDTH}} delta digest failed: {error}")
            had_failure = True
        if not entry["page_count_equal"]:
            had_failure = True
            print(
                f"{name:<{FIXTURE_COLUMN_WIDTH}} page-count mismatch: "
                f"Python {entry['page_count_expected']}, Rust {entry['page_count_actual']}"
            )
        for page in entry["pages"]:
            if page["status"] != "compared":
                had_failure = True
                print(
                    f"{name:<{FIXTURE_COLUMN_WIDTH}} "
                    f"{page['page_number']:>4} {page['status']}"
                )
                continue
            compared_pages.append(page)
            unsupported = unsupported_api_names(page)
            if unsupported:
                had_failure = True
            print(
                f"{name:<{FIXTURE_COLUMN_WIDTH}} "
                f"{page['page_number']:>4} "
                f"{page['chars']['text_ratio']:>6.3f}/{page['chars']['box_ratio']:<7.3f} "
                f"{page['words']['ratio']:>8.3f} "
                f"{'yes' if page['page_text']['equal'] else 'no':>6} "
                f"{page['tables']['cell_ratio']:>8.3f} "
                f"{len(unsupported):>11}"
            )

    if compared_pages:
        print("-" * (FIXTURE_COLUMN_WIDTH + 57))
        print(
            "mean       "
            f"chars(text/box) "
            f"{sum(p['chars']['text_ratio'] for p in compared_pages) / len(compared_pages):.3f}"
            f"/{sum(p['chars']['box_ratio'] for p in compared_pages) / len(compared_pages):.3f}  "
            f"words "
            f"{sum(p['words']['ratio'] for p in compared_pages) / len(compared_pages):.3f}  "
            f"text {sum(1 for p in compared_pages if p['page_text']['equal'])}"
            f"/{len(compared_pages)}  "
            f"tables "
            f"{sum(p['tables']['cell_ratio'] for p in compared_pages) / len(compared_pages):.3f}  "
            f"unsupported "
            f"{sum(len(unsupported_api_names(p)) for p in compared_pages)}"
        )

    scoped_registry = approved_deltas.Registry(
        version=registry.version,
        commit=registry.commit,
        deltas=tuple(
            delta for delta in registry.deltas if delta.fixture in report
        ),
    )
    gate = approved_deltas.evaluate(tuple(observed_deltas), scoped_registry)
    for delta in gate.unregistered:
        print(
            "unregistered delta: "
            f"{delta.fixture} page {delta.page} {delta.api} "
            f"upstream={delta.upstream_sha256} rust={delta.rust_sha256}"
        )
    for delta in gate.stale:
        print(
            "stale approved delta: "
            f"{delta.identifier} {delta.fixture} page {delta.page} {delta.api}"
        )
    print(
        "approved-delta gate: "
        f"{len(gate.approved)} approved, "
        f"{len(gate.unregistered)} unregistered, {len(gate.stale)} stale"
    )
    if gate.exit_code:
        had_failure = True

    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump(report, handle, indent=1)

    return 1 if had_failure else 0


if __name__ == "__main__":
    sys.exit(main())
