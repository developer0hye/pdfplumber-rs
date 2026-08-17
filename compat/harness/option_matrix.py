"""Pinned-upstream text/table option matrix for PARITY-011.

The catalog is deliberately declarative: every case names the public keyword
that it covers, supplies a non-default value, and records the fixture/page used
to produce its reference value.  The runner stores complete normalized output,
not a pass percentage or a hand-selected projection.
"""

from __future__ import annotations

import hashlib
import json
import logging
import math
import warnings
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any, Literal, Mapping

from compat.harness import lockfile, upstream


SCHEMA_VERSION: int = 1
GENERATION_COMMAND: str = "scripts/generate_option_matrix.py"

Domain = Literal["text", "table"]


@dataclass(frozen=True)
class Case:
    identifier: str
    domain: Domain
    api: str
    fixture: Path
    page_number: int
    options: Mapping[str, object]
    covers: tuple[str, ...]
    arguments: Mapping[str, object]


def _fixture(relative_path: str) -> Path:
    return upstream.REPO_ROOT / relative_path


TEXT_FIXTURE: Path = _fixture("tests/fixtures/generated/basic_text.pdf")
TABLE_FIXTURE: Path = _fixture("tests/fixtures/generated/table_lattice.pdf")
BORDERLESS_TABLE_FIXTURE: Path = _fixture(
    "tests/fixtures/generated/table_borderless.pdf"
)


def _case(
    identifier: str,
    domain: Domain,
    api: str,
    fixture: Path,
    options: Mapping[str, object],
    *covers: str,
    arguments: Mapping[str, object] | None = None,
) -> Case:
    return Case(
        identifier=identifier,
        domain=domain,
        api=api,
        fixture=fixture,
        page_number=1,
        options=dict(options),
        covers=tuple(covers),
        arguments={} if arguments is None else dict(arguments),
    )


def _text_cases() -> list[Case]:
    cases: list[Case] = []

    def add(
        identifier: str,
        api: str,
        options: Mapping[str, object],
        *covers: str,
        arguments: Mapping[str, object] | None = None,
    ) -> None:
        cases.append(
            _case(
                f"text.{identifier}",
                "text",
                api,
                TEXT_FIXTURE,
                options,
                *covers,
                arguments=arguments,
            )
        )

    word_variants: tuple[
        tuple[str, str, object, Mapping[str, object]], ...
    ] = (
        ("x_tolerance", "x_tolerance", 1, {}),
        ("y_tolerance", "y_tolerance", 1, {}),
        ("x_tolerance_ratio", "x_tolerance_ratio", 0.4, {}),
        ("y_tolerance_ratio", "y_tolerance_ratio", 0.4, {}),
        ("keep_blank_chars", "keep_blank_chars", True, {}),
        ("use_text_flow", "use_text_flow", True, {}),
        ("vertical_ttb", "vertical_ttb", False, {}),
        ("horizontal_ltr", "horizontal_ltr", False, {}),
        ("line_dir", "line_dir", "btt", {}),
        ("char_dir", "char_dir", "rtl", {}),
        ("line_dir_rotated", "line_dir_rotated", "rtl", {}),
        ("char_dir_rotated", "char_dir_rotated", "btt", {}),
        ("extra_attrs", "extra_attrs", ["fontname"], {}),
        ("split_at_punctuation", "split_at_punctuation.true", True, {}),
        ("split_at_punctuation", "split_at_punctuation.custom", ",.;", {}),
        ("expand_ligatures", "expand_ligatures", False, {}),
    )
    textmap_variants: tuple[
        tuple[str, str, object, Mapping[str, object]], ...
    ] = (
        ("layout", "layout", True, {}),
        ("layout_width", "layout_width", 500, {"layout": True}),
        ("layout_height", "layout_height", 700, {"layout": True}),
        (
            "layout_width_chars",
            "layout_width_chars",
            70,
            {"layout": True},
        ),
        (
            "layout_height_chars",
            "layout_height_chars",
            50,
            {"layout": True},
        ),
        (
            "layout_bbox",
            "layout_bbox",
            {"$page_bbox_inset": 20},
            {"layout": True},
        ),
        ("x_density", "x_density", 8, {"layout": True}),
        ("y_density", "y_density", 14, {"layout": True}),
        ("x_shift", "x_shift", 2, {"layout": True}),
        ("y_shift", "y_shift", 2, {"layout": True}),
        ("char_dir_render", "char_dir_render", "rtl", {}),
        ("line_dir_render", "line_dir_render", "btt", {}),
    )

    for option, suffix, value, companions in word_variants:
        options: dict[str, object] = dict(companions)
        options[option] = value
        add(
            f"extract_words.{suffix}",
            "extract_words",
            options,
            f"extract_words.{option}",
        )
    add(
        "extract_words.return_chars",
        "extract_words",
        {"return_chars": True},
        "extract_words.return_chars",
    )

    page_variants = word_variants + textmap_variants
    for api in ("extract_text", "extract_text_lines", "search"):
        for option, suffix, value, companions in page_variants:
            options = dict(companions)
            options[option] = value
            add(
                f"{api}.{suffix}",
                api,
                options,
                f"{api}.{option}",
                arguments={"pattern": "The"} if api == "search" else None,
            )

    add(
        "extract_text_simple.x_tolerance",
        "extract_text_simple",
        {"x_tolerance": 1},
        "extract_text_simple.x_tolerance",
    )
    add(
        "extract_text_simple.y_tolerance",
        "extract_text_simple",
        {"y_tolerance": 1},
        "extract_text_simple.y_tolerance",
    )
    add(
        "extract_text_lines.return_chars",
        "extract_text_lines",
        {"return_chars": False},
        "extract_text_lines.return_chars",
    )
    add(
        "extract_text_lines.strip",
        "extract_text_lines",
        {"strip": False},
        "extract_text_lines.strip",
    )
    add(
        "search.regex",
        "search",
        {"regex": False},
        "search.regex",
        arguments={"pattern": "The"},
    )
    add(
        "search.case",
        "search",
        {"case": False},
        "search.case",
        arguments={"pattern": "the"},
    )
    add(
        "search.main_group",
        "search",
        {"main_group": 1},
        "search.main_group",
        arguments={"pattern": "(The)"},
    )
    add(
        "search.return_chars",
        "search",
        {"return_chars": False},
        "search.return_chars",
        arguments={"pattern": "The"},
    )
    add(
        "search.return_groups",
        "search",
        {"return_groups": False},
        "search.return_groups",
        arguments={"pattern": "(The)"},
    )
    add(
        "utils.extract_text.presorted",
        "utils.extract_text",
        {"presorted": True},
        "utils.extract_text.presorted",
    )
    return cases


def _table_cases() -> list[Case]:
    cases: list[Case] = []

    def add(
        identifier: str,
        options: Mapping[str, object],
        *covers: str,
        fixture: Path = TABLE_FIXTURE,
    ) -> None:
        cases.append(
            _case(
                f"table.{identifier}",
                "table",
                "extract_tables",
                fixture,
                options,
                *(covers or (identifier,)),
            )
        )

    # Enumerated strategies get one case per documented non-default value.  The
    # explicit strategy cases derive their guide lines from the current page
    # bounds, rather than encoding an expected table or fixture-specific output.
    add(
        "vertical_strategy.lines_strict",
        {"vertical_strategy": "lines_strict"},
        "vertical_strategy",
    )
    add(
        "vertical_strategy.text",
        {"vertical_strategy": "text"},
        "vertical_strategy",
        fixture=BORDERLESS_TABLE_FIXTURE,
    )
    add(
        "vertical_strategy.explicit",
        {
            "vertical_strategy": "explicit",
            "explicit_vertical_lines": {"$page_vertical_bounds": True},
        },
        "vertical_strategy",
    )
    add(
        "horizontal_strategy.lines_strict",
        {"horizontal_strategy": "lines_strict"},
        "horizontal_strategy",
    )
    add(
        "horizontal_strategy.text",
        {"horizontal_strategy": "text"},
        "horizontal_strategy",
        fixture=BORDERLESS_TABLE_FIXTURE,
    )
    add(
        "horizontal_strategy.explicit",
        {
            "horizontal_strategy": "explicit",
            "explicit_horizontal_lines": {"$page_horizontal_bounds": True},
        },
        "horizontal_strategy",
    )

    add("explicit_vertical_lines", {"explicit_vertical_lines": {"$page_vertical_bounds": True}})
    add(
        "explicit_horizontal_lines",
        {"explicit_horizontal_lines": {"$page_horizontal_bounds": True}},
    )
    add("snap_tolerance", {"snap_tolerance": 1})
    add("snap_x_tolerance", {"snap_x_tolerance": 1})
    add("snap_y_tolerance", {"snap_y_tolerance": 1})
    add("join_tolerance", {"join_tolerance": 1})
    add("join_x_tolerance", {"join_x_tolerance": 1})
    add("join_y_tolerance", {"join_y_tolerance": 1})
    add("edge_min_length", {"edge_min_length": 1})
    add("edge_min_length_prefilter", {"edge_min_length_prefilter": 0.5})
    add("min_words_vertical", {"min_words_vertical": 2})
    add("min_words_horizontal", {"min_words_horizontal": 2})
    add("intersection_tolerance", {"intersection_tolerance": 1})
    add("intersection_x_tolerance", {"intersection_x_tolerance": 1})
    add("intersection_y_tolerance", {"intersection_y_tolerance": 1})
    add("text_tolerance", {"text_tolerance": 1})
    add("text_x_tolerance", {"text_x_tolerance": 1})
    add("text_y_tolerance", {"text_y_tolerance": 1})

    prefixed_values: tuple[tuple[str, object, Mapping[str, object]], ...] = (
        ("x_tolerance_ratio", 0.4, {}),
        ("y_tolerance_ratio", 0.4, {}),
        ("keep_blank_chars", True, {}),
        ("use_text_flow", True, {}),
        ("vertical_ttb", False, {}),
        ("horizontal_ltr", False, {}),
        ("line_dir", "btt", {}),
        ("char_dir", "rtl", {}),
        ("line_dir_rotated", "rtl", {}),
        ("char_dir_rotated", "btt", {}),
        ("extra_attrs", ["fontname"], {}),
        ("split_at_punctuation", True, {}),
        ("expand_ligatures", False, {}),
        ("layout", True, {}),
        ("layout_width", 500, {}),
        ("layout_height", 700, {}),
        ("layout_width_chars", 70, {}),
        ("layout_height_chars", 50, {}),
        ("layout_bbox", {"$page_bbox_inset": 20}, {}),
        ("x_density", 8, {"text_layout": True}),
        ("y_density", 14, {"text_layout": True}),
        ("x_shift", 2, {"text_layout": True}),
        ("y_shift", 2, {"text_layout": True}),
        ("char_dir_render", "rtl", {}),
        ("line_dir_render", "btt", {}),
        ("presorted", True, {}),
    )
    for option, value, companions in prefixed_values:
        keyword: str = f"text_{option}"
        settings: dict[str, object] = dict(companions)
        settings[keyword] = value
        add(keyword, settings, keyword)

    return cases


def cases() -> tuple[Case, ...]:
    """Return the stable option catalog in snapshot order."""
    return tuple(_text_cases() + _table_cases())


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def snapshot_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "snapshots"
        / f"{target.project}-v{target.version}-option-matrix.json"
    )


def _resolve(value: object, page: Any) -> object:
    if isinstance(value, dict):
        if set(value) == {"$page_bbox_inset"}:
            inset: object = value["$page_bbox_inset"]
            if not isinstance(inset, (int, float)) or isinstance(inset, bool):
                raise TypeError("$page_bbox_inset must be numeric")
            x0, top, x1, bottom = page.bbox
            return (x0 + inset, top + inset, x1 - inset, bottom - inset)
        if value == {"$page_vertical_bounds": True}:
            return [page.bbox[0], page.bbox[2]]
        if value == {"$page_horizontal_bounds": True}:
            return [page.bbox[1], page.bbox[3]]
        return {key: _resolve(item, page) for key, item in value.items()}
    if isinstance(value, list):
        return [_resolve(item, page) for item in value]
    if isinstance(value, tuple):
        return tuple(_resolve(item, page) for item in value)
    return value


def _normalize(value: object) -> object:
    """Retain values and container types in deterministic JSON-safe form."""
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        if math.isfinite(value):
            return value
        return {"$type": "float", "value": repr(value)}
    if isinstance(value, Decimal):
        return {"$type": "decimal.Decimal", "value": str(value)}
    if isinstance(value, tuple):
        return {"$type": "tuple", "items": [_normalize(item) for item in value]}
    if isinstance(value, list):
        return [_normalize(item) for item in value]
    if isinstance(value, dict):
        if all(isinstance(key, str) for key in value):
            return {
                key: _normalize(value[key])
                for key in sorted(value)  # type: ignore[type-var]
            }
        return {
            "$type": "dict",
            "items": [
                [_normalize(key), _normalize(item)] for key, item in value.items()
            ],
        }
    if isinstance(value, set):
        items: list[object] = [_normalize(item) for item in value]
        return {"$type": "set", "items": sorted(items, key=repr)}
    value_type: type[object] = type(value)
    raise TypeError(
        "option-matrix output contains unsupported type "
        f"{value_type.__module__}.{value_type.__qualname__}"
    )


def _invoke(root_package: Any, case: Case, page: Any, options: dict[str, object]) -> object:
    if case.api == "extract_text":
        return page.extract_text(**options)
    if case.api == "extract_text_simple":
        return page.extract_text_simple(**options)
    if case.api == "utils.extract_text":
        return root_package.utils.extract_text(page.chars, **options)
    if case.api == "extract_words":
        return page.extract_words(**options)
    if case.api == "extract_text_lines":
        return page.extract_text_lines(**options)
    if case.api == "search":
        arguments: dict[str, object] = {
            key: _resolve(value, page) for key, value in case.arguments.items()
        }
        return page.search(**arguments, **options)
    if case.api == "extract_tables":
        return page.extract_tables(options)
    raise ValueError(f"unknown option-matrix API: {case.api}")


def _run_case(root_package: Any, case: Case) -> dict[str, object]:
    fixture_relative: str = case.fixture.relative_to(upstream.REPO_ROOT).as_posix()
    base: dict[str, object] = {
        "id": case.identifier,
        "domain": case.domain,
        "api": case.api,
        "fixture_path": fixture_relative,
        "fixture_sha256": file_sha256(case.fixture),
        "page_number": case.page_number,
        "covers": list(case.covers),
    }

    captured: list[warnings.WarningMessage]
    log_records: list[logging.LogRecord] = []

    class CaptureHandler(logging.Handler):
        def emit(self, record: logging.LogRecord) -> None:
            log_records.append(record)

    package_logger: logging.Logger = logging.getLogger("pdfplumber")
    handler: CaptureHandler = CaptureHandler()
    old_propagate: bool = package_logger.propagate
    package_logger.addHandler(handler)
    package_logger.propagate = False
    try:
        with root_package.open(case.fixture) as pdf:
            page: Any = pdf.pages[case.page_number - 1]
            resolved_options: dict[str, object] = {
                key: _resolve(value, page) for key, value in case.options.items()
            }
            base["options"] = _normalize(resolved_options)
            base["arguments"] = _normalize(
                {
                    key: _resolve(value, page)
                    for key, value in case.arguments.items()
                }
            )
            with warnings.catch_warnings(record=True) as captured:
                warnings.simplefilter("always")
                result: object = _invoke(
                    root_package, case, page, resolved_options
                )
        base["status"] = "ok"
        base["warnings"] = [
            {
                "category": (
                    f"{item.category.__module__}.{item.category.__qualname__}"
                ),
                "message": str(item.message),
            }
            for item in captured
        ]
        base["logs"] = [
            {
                "level": record.levelname,
                "logger": record.name,
                "message": record.getMessage(),
            }
            for record in log_records
        ]
        base["result"] = _normalize(result)
    except Exception as error:  # noqa: BLE001 - errors are reference behavior
        error_type: type[BaseException] = type(error)
        base["status"] = "error"
        base["error"] = {
            "type": f"{error_type.__module__}.{error_type.__qualname__}",
            "message": str(error),
        }
    finally:
        package_logger.removeHandler(handler)
        package_logger.propagate = old_propagate
    return base


def build(root_package: Any) -> dict[str, object]:
    """Execute every case against the already-verified pinned package."""
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
    records: list[dict[str, object]] = [
        _run_case(root_package, case) for case in cases()
    ]
    outputs: dict[str, object] = {}
    for record in records:
        if record.get("status") != "ok":
            continue
        result: object = record.pop("result")
        canonical: str = json.dumps(
            result,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
        digest: str = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        outputs.setdefault(digest, result)
        record["result"] = {"$ref": digest}

    return {
        "schema_version": SCHEMA_VERSION,
        "target": {
            "project": target.project,
            "version": target.version,
            "tag": target.tag,
            "commit": target.commit,
            "repository": target.repository,
        },
        "python_version": environment.python_version,
        "lockfile_sha256": lockfile.digest(),
        "generated_by": GENERATION_COMMAND,
        "outputs": outputs,
        "cases": records,
    }
