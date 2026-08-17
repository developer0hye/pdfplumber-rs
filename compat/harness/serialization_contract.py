"""Exact JSON/CSV serialization contracts for PARITY-013."""

from __future__ import annotations

import hashlib
import io
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType

from compat.harness import lockfile, upstream


SCHEMA_VERSION: int = 1
GENERATION_COMMAND: str = "compat/serialization_contract.py --write-reference"
FIXTURE: Path = Path(
    "crates/pdfplumber/tests/fixtures/pdfs/issue-67-example.pdf"
)
BASIC_FIXTURE: Path = Path("tests/fixtures/generated/basic_text.pdf")
FIXTURES: tuple[Path, ...] = (FIXTURE, BASIC_FIXTURE)


@dataclass(frozen=True)
class Observation:
    returned: object
    stream: str | None = None


Call = Callable[[ModuleType, Path], Observation]


@dataclass(frozen=True)
class Case:
    identifier: str
    category: str
    surface: str
    fixture: Path
    callable_name: str
    invocation: dict[str, object]
    call: Call


def contract_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "contracts"
        / f"{target.project}-v{target.version}-serialization.json"
    )


def build(package: ModuleType) -> dict[str, object]:
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
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
        "generated_by": GENERATION_COMMAND,
        "resources": {
            fixture.as_posix(): {
                "sha256": hashlib.sha256(
                    (upstream.REPO_ROOT / fixture).read_bytes()
                ).hexdigest()
            }
            for fixture in FIXTURES
        },
        "cases": [
            run_case(case, package, upstream.REPO_ROOT / case.fixture)
            for case in cases()
        ],
    }


def run_case(case: Case, package: ModuleType, fixture: Path) -> dict[str, object]:
    record: dict[str, object] = {
        "id": case.identifier,
        "category": case.category,
        "surface": case.surface,
        "callable": case.callable_name,
        "invocation": case.invocation,
    }
    try:
        observation: Observation = case.call(package, fixture)
    except Exception as error:  # noqa: BLE001 - exception behavior is contractual
        record["outcome"] = {
            "kind": "exception",
            "type": _type_name(type(error)),
            "message": _stable_text(str(error)),
            "args": [_normalize(argument) for argument in error.args],
        }
    else:
        outcome: dict[str, object] = {
            "kind": "return",
            "return": _observed(observation.returned),
        }
        if observation.stream is not None:
            outcome["stream"] = _observed(observation.stream)
        record["outcome"] = outcome
    return record


def _observed(value: object) -> dict[str, object]:
    observed: dict[str, object] = {
        "type": _type_name(type(value)),
        "value": _normalize(value),
    }
    if isinstance(value, str):
        observed["sha256"] = hashlib.sha256(value.encode("utf-8")).hexdigest()
    return observed


def _normalize(value: object) -> object:
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, bytes):
        return {"bytes_hex": value.hex()}
    if isinstance(value, Mapping):
        return {str(key): _normalize(item) for key, item in value.items()}
    if isinstance(value, Sequence):
        return [_normalize(item) for item in value]
    return {"type": _type_name(type(value)), "repr": _stable_text(repr(value))}


def _type_name(value_type: type[object]) -> str:
    return f"{value_type.__module__}.{value_type.__qualname__}"


def _stable_text(value: str) -> str:
    return value.replace(str(upstream.REPO_ROOT), "<repo>")


def _serialize(
    surface: str,
    method_name: str,
    options: dict[str, object],
    *,
    use_stream: bool = False,
) -> Call:
    def call(package: ModuleType, fixture: Path) -> Observation:
        with package.open(fixture, pages=[1]) as pdf:
            target = pdf if surface == "pdf" else pdf.pages[0]
            method = getattr(target, method_name)
            if use_stream:
                stream = io.StringIO()
                returned = method(stream=stream, **options)
                return Observation(returned=returned, stream=stream.getvalue())
            return Observation(returned=method(**options))

    return call


def _case(
    identifier: str,
    surface: str,
    method_name: str,
    options: dict[str, object] | None = None,
    *,
    fixture: Path = FIXTURE,
    use_stream: bool = False,
) -> Case:
    category: str = identifier.split(".", 1)[0]
    actual_options: dict[str, object] = {} if options is None else options
    invocation: dict[str, object] = {
        "input": fixture.as_posix(),
        "pages": [1],
        "options": actual_options,
    }
    if use_stream:
        invocation["stream"] = "StringIO"
    class_name: str = "PDF" if surface == "pdf" else "Page"
    return Case(
        identifier=identifier,
        category=category,
        surface=surface,
        fixture=fixture,
        callable_name=f"pdfplumber.{surface}.{class_name}.{method_name}",
        invocation=invocation,
        call=_serialize(
            surface,
            method_name,
            actual_options,
            use_stream=use_stream,
        ),
    )


def cases() -> tuple[Case, ...]:
    records: list[Case] = [
        _case(
            "csv.invalid_both_filters",
            "pdf",
            "to_csv",
            {"include_attrs": ["text"], "exclude_attrs": ["matrix"]},
        ),
        _case(
            "csv.invalid_required_exclude",
            "page",
            "to_csv",
            {"exclude_attrs": ["object_type"]},
        ),
        _case("csv.page_default", "page", "to_csv"),
        _case("csv.page_stream", "page", "to_csv", use_stream=True),
        _case(
            "csv.pdf_basic_text_default",
            "pdf",
            "to_csv",
            fixture=BASIC_FIXTURE,
        ),
        _case(
            "csv.pdf_chars_only",
            "pdf",
            "to_csv",
            {"object_types": ["char"]},
        ),
        _case("csv.pdf_default", "pdf", "to_csv"),
        _case(
            "csv.pdf_exclude",
            "pdf",
            "to_csv",
            {"exclude_attrs": ["matrix", "stream"]},
        ),
        _case(
            "csv.pdf_include",
            "pdf",
            "to_csv",
            {"include_attrs": ["page_number", "x0", "text"]},
        ),
        _case(
            "csv.pdf_no_objects",
            "pdf",
            "to_csv",
            {"object_types": []},
        ),
        _case("csv.pdf_precision_0", "pdf", "to_csv", {"precision": 0}),
        _case("csv.pdf_precision_3", "pdf", "to_csv", {"precision": 3}),
        _case("csv.pdf_stream", "pdf", "to_csv", use_stream=True),
        _case(
            "json.invalid_both_filters",
            "pdf",
            "to_json",
            {"include_attrs": ["text"], "exclude_attrs": ["matrix"]},
        ),
        _case(
            "json.invalid_required_exclude",
            "page",
            "to_json",
            {"exclude_attrs": ["object_type"]},
        ),
        _case("json.page_default", "page", "to_json"),
        _case(
            "json.page_precision_3",
            "page",
            "to_json",
            {"precision": 3},
        ),
        _case("json.page_stream", "page", "to_json", use_stream=True),
        _case(
            "json.pdf_basic_text_default",
            "pdf",
            "to_json",
            fixture=BASIC_FIXTURE,
        ),
        _case(
            "json.pdf_chars_only",
            "pdf",
            "to_json",
            {"object_types": ["char"]},
        ),
        _case("json.pdf_default", "pdf", "to_json"),
        _case(
            "json.pdf_exclude",
            "pdf",
            "to_json",
            {"exclude_attrs": ["matrix", "stream"]},
        ),
        _case(
            "json.pdf_include",
            "pdf",
            "to_json",
            {"include_attrs": ["page_number", "x0", "text"]},
        ),
        _case("json.pdf_indent_2", "pdf", "to_json", {"indent": 2}),
        _case(
            "json.pdf_no_objects",
            "pdf",
            "to_json",
            {"object_types": []},
        ),
        _case("json.pdf_precision_0", "pdf", "to_json", {"precision": 0}),
        _case("json.pdf_precision_3", "pdf", "to_json", {"precision": 3}),
        _case("json.pdf_stream", "pdf", "to_json", use_stream=True),
    ]
    return tuple(sorted(records, key=lambda case: case.identifier))
