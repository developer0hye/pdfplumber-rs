"""Executable public-call contracts for pinned Python pdfplumber.

The API snapshot records signatures. These cases exercise Python's actual call
binding and validation so a future compatibility package must also match the
accepted argument forms, normalized return values, and exception categories.
"""

from __future__ import annotations

import dataclasses
import hashlib
import importlib
import io
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Any

from compat.harness import lockfile, upstream


SCHEMA_VERSION: int = 1
PDF_FIXTURE: Path = Path("tests/fixtures/downloaded/pdffill-demo.pdf")
_MEMORY_ADDRESS: re.Pattern[str] = re.compile(r"0x[0-9a-fA-F]+")

Call = Callable[[ModuleType, Path], object]
Project = Callable[[object], object]


@dataclass(frozen=True)
class Case:
    id: str
    category: str
    callable_name: str
    invocation: dict[str, object]
    call: Call
    project: Project = lambda value: value
    projection_name: str | None = None


def contract_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "contracts"
        / f"{target.project}-v{target.version}-calls.json"
    )


def build(package: ModuleType) -> dict[str, object]:
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
    fixture: Path = upstream.REPO_ROOT / PDF_FIXTURE
    cases: list[dict[str, object]] = [
        run_case(case, package, fixture) for case in sorted(_cases(), key=lambda c: c.id)
    ]

    return {
        "schema_version": SCHEMA_VERSION,
        "target": {
            "project": target.project,
            "version": target.version,
            "tag": target.tag,
            "commit": target.commit,
            "repository": target.repository,
        },
        "environment": {
            "python_version": environment.python_version,
            "lockfile_sha256": lockfile.digest(),
        },
        "resources": {
            PDF_FIXTURE.as_posix(): {
                "sha256": hashlib.sha256(fixture.read_bytes()).hexdigest()
            }
        },
        "cases": cases,
    }


def run_case(case: Case, package: ModuleType, fixture: Path) -> dict[str, object]:
    record: dict[str, object] = {
        "id": case.id,
        "category": case.category,
        "callable": case.callable_name,
        "invocation": case.invocation,
    }
    if case.projection_name is not None:
        record["result_projection"] = case.projection_name

    try:
        value: object = case.call(package, fixture)
    except Exception as error:
        record["outcome"] = {
            "kind": "exception",
            "type": _type_name(type(error)),
            "message": _stable_text(str(error)),
        }
        return record

    record["outcome"] = {
        "kind": "return",
        "type": _type_name(type(value)),
        "value": normalize(case.project(value)),
    }
    return record


def normalize(value: object) -> object:
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, bytes):
        return {"bytes_hex": value.hex()}
    if isinstance(value, Mapping):
        return {
            str(key): normalize(item)
            for key, item in sorted(value.items(), key=lambda pair: str(pair[0]))
        }
    if isinstance(value, Sequence):
        return [normalize(item) for item in value]
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return normalize(dataclasses.asdict(value))
    return {"type": _type_name(type(value)), "repr": _stable_text(repr(value))}


def _cases() -> list[Case]:
    cluster_values: list[int] = [1, 2, 5, 6]
    return [
        Case(
            id="defaults.cluster_list",
            category="defaults",
            callable_name="pdfplumber.utils.cluster_list",
            invocation={"args": [[1, 2, 3, 4]], "kwargs": {}},
            call=lambda package, _: package.utils.cluster_list([1, 2, 3, 4]),
        ),
        Case(
            id="defaults.table_settings",
            category="defaults",
            callable_name="pdfplumber.table.TableSettings",
            invocation={"args": [], "kwargs": {}},
            call=lambda package, _: _module(package, "table").TableSettings(),
        ),
        Case(
            id="exception.empty_pdf",
            category="exception_types",
            callable_name="pdfplumber.open",
            invocation={"args": [{"bytes_hex": ""}], "kwargs": {}},
            call=lambda package, _: package.open(io.BytesIO(b"")),
        ),
        Case(
            id="exception.table_settings_strategy",
            category="exception_types",
            callable_name="pdfplumber.table.TableSettings",
            invocation={"args": [], "kwargs": {"vertical_strategy": "bogus"}},
            call=lambda package, _: _module(package, "table").TableSettings(
                vertical_strategy="bogus"
            ),
        ),
        Case(
            id="invalid.missing_required",
            category="invalid_arguments",
            callable_name="pdfplumber.utils.get_bbox_overlap",
            invocation={"args": [[0, 0, 1, 1]], "kwargs": {}},
            call=lambda package, _: package.utils.get_bbox_overlap((0, 0, 1, 1)),
        ),
        Case(
            id="invalid.too_many_positional",
            category="invalid_arguments",
            callable_name="pdfplumber.utils.cluster_list",
            invocation={"args": [[1], 0, 2], "kwargs": {}},
            call=lambda package, _: package.utils.cluster_list([1], 0, 2),
        ),
        Case(
            id="invalid.unexpected_keyword",
            category="invalid_arguments",
            callable_name="pdfplumber.utils.cluster_list",
            invocation={"args": [[1]], "kwargs": {"surprise": True}},
            call=lambda package, _: package.utils.cluster_list([1], surprise=True),
        ),
        Case(
            id="keyword.cluster_list",
            category="keyword_arguments",
            callable_name="pdfplumber.utils.cluster_list",
            invocation={
                "args": [],
                "kwargs": {"xs": cluster_values, "tolerance": 1},
            },
            call=lambda package, _: package.utils.cluster_list(
                xs=cluster_values, tolerance=1
            ),
        ),
        Case(
            id="keyword_only.page_extract_text",
            category="keyword_only_arguments",
            callable_name="pdfplumber.page.Page.extract_text",
            invocation={
                "receiver": f"{PDF_FIXTURE.as_posix()} page 1",
                "args": [],
                "kwargs": {"layout": True},
            },
            call=lambda package, fixture: _page_extract_text(
                package, fixture, keyword=True
            ),
            project=_text_summary,
            projection_name=(
                "utf8_sha256, length, line_count, pdfill_heading_present"
            ),
        ),
        Case(
            id="keyword_only.page_extract_text_rejects_positional",
            category="keyword_only_arguments",
            callable_name="pdfplumber.page.Page.extract_text",
            invocation={
                "receiver": f"{PDF_FIXTURE.as_posix()} page 1",
                "args": [True],
                "kwargs": {},
            },
            call=lambda package, fixture: _page_extract_text(
                package, fixture, keyword=False
            ),
        ),
        Case(
            id="positional.cluster_list",
            category="positional_arguments",
            callable_name="pdfplumber.utils.cluster_list",
            invocation={"args": [cluster_values, 1], "kwargs": {}},
            call=lambda package, _: package.utils.cluster_list(cluster_values, 1),
        ),
        Case(
            id="positional_only.ctm_count",
            category="positional_arguments",
            callable_name="pdfplumber.ctm.CTM.count",
            invocation={
                "receiver": "CTM(1, 0, 0, 1, 0, 0)",
                "args": [0],
                "kwargs": {},
            },
            call=lambda package, _: _module(package, "ctm")
            .CTM(1, 0, 0, 1, 0, 0)
            .count(0),
        ),
    ]


def _module(package: ModuleType, suffix: str) -> ModuleType:
    return importlib.import_module(f"{package.__name__}.{suffix}")


def _page_extract_text(
    package: ModuleType, fixture: Path, *, keyword: bool
) -> object:
    with package.open(fixture) as pdf:
        page: Any = pdf.pages[0]
        if keyword:
            return page.extract_text(layout=True)
        return page.extract_text(True)


def _text_summary(value: object) -> object:
    text: str = str(value)
    return {
        "utf8_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
        "length": len(text),
        "line_count": len(text.splitlines()),
        "pdfill_heading_present": "PDFill:" in text and "Drawing" in text,
    }


def _type_name(value_type: type[object]) -> str:
    return f"{value_type.__module__}.{value_type.__qualname__}"


def _stable_text(value: str) -> str:
    return _MEMORY_ADDRESS.sub("<address>", value)
