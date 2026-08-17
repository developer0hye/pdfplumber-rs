"""Exceptional, warning, repair, and resource contracts for PARITY-012."""

from __future__ import annotations

import dataclasses
import hashlib
import importlib
import io
import logging
import os
import re
import tempfile
import warnings
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Any

from compat.harness import lockfile, upstream


SCHEMA_VERSION: int = 1
BASIC_FIXTURE: Path = Path("tests/fixtures/generated/basic_text.pdf")
ANNOTATION_FIXTURE: Path = Path(
    "crates/pdfplumber/tests/fixtures/pdfs/annotations-unicode-issues.pdf"
)
PASSWORD_FIXTURE: Path = Path(
    "crates/pdfplumber/tests/fixtures/pdfs/password-example.pdf"
)

_MEMORY_ADDRESS: re.Pattern[str] = re.compile(r"0x[0-9a-fA-F]{6,}")

REPAIR_HELPER_SOURCE: str = """#!/usr/bin/env python3
import pathlib
import sys

name = pathlib.Path(sys.argv[0]).name
arguments = sys.argv[1:]
if "fail" in name:
    sys.stderr.write("synthetic repair failure\\n")
    raise SystemExit(7)
if "prepress" in name and "-dPDFSETTINGS=/prepress" not in arguments:
    sys.stderr.write("missing prepress setting\\n")
    raise SystemExit(8)
source = arguments[-1]
payload = sys.stdin.buffer.read() if source == "-" else pathlib.Path(source).read_bytes()
sys.stdout.buffer.write(payload)
"""


def _cyclic_metadata_pdf() -> bytes:
    objects: tuple[bytes, ...] = (
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>",
        b"<< /Loop 4 0 R >>",
    )
    parts: list[bytes] = [b"%PDF-1.4\n"]
    offsets: list[int] = []
    for number, body in enumerate(objects, start=1):
        offsets.append(sum(map(len, parts)))
        parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
    xref_offset: int = sum(map(len, parts))
    parts.extend(
        [
            f"xref\n0 {len(objects) + 1}\n".encode(),
            b"0000000000 65535 f \n",
            *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
            (
                "trailer\n<< /Root 1 0 R /Info 4 0 R /Size 5 >>\n"
                f"startxref\n{xref_offset}\n%%EOF\n"
            ).encode(),
        ]
    )
    return b"".join(parts)


@dataclass(frozen=True)
class Resources:
    root: Path
    basic: Path
    annotation: Path
    password: Path
    cyclic_metadata: bytes


@dataclass
class Phase:
    current: str

    def mark(self, value: str) -> None:
        self.current = value


Call = Callable[[ModuleType, Resources, Phase], object]


@dataclass(frozen=True)
class Case:
    identifier: str
    category: str
    callable_name: str
    invocation: dict[str, object]
    initial_phase: str
    call: Call


def contract_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "contracts"
        / f"{target.project}-v{target.version}-error-behavior.json"
    )


def _resources() -> Resources:
    root: Path = upstream.REPO_ROOT
    return Resources(
        root=root,
        basic=root / BASIC_FIXTURE,
        annotation=root / ANNOTATION_FIXTURE,
        password=root / PASSWORD_FIXTURE,
        cyclic_metadata=_cyclic_metadata_pdf(),
    )


def build(package: ModuleType) -> dict[str, object]:
    resources: Resources = _resources()
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
    fixture_resources: dict[str, dict[str, str]] = {
        path.as_posix(): {
            "sha256": hashlib.sha256((resources.root / path).read_bytes()).hexdigest()
        }
        for path in (BASIC_FIXTURE, ANNOTATION_FIXTURE, PASSWORD_FIXTURE)
    }
    fixture_resources["inline:cyclic-metadata.pdf"] = {
        "sha256": hashlib.sha256(resources.cyclic_metadata).hexdigest()
    }
    fixture_resources["inline:repair-helper.py"] = {
        "sha256": hashlib.sha256(REPAIR_HELPER_SOURCE.encode()).hexdigest()
    }

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
        "resources": fixture_resources,
        "cases": [run_case(case, package, resources) for case in cases()],
    }


class _CaptureHandler(logging.Handler):
    def __init__(self, records: list[logging.LogRecord]):
        super().__init__()
        self.records = records

    def emit(self, record: logging.LogRecord) -> None:
        self.records.append(record)


def run_case(
    case: Case, package: ModuleType, resources: Resources
) -> dict[str, object]:
    phase = Phase(case.initial_phase)
    log_records: list[logging.LogRecord] = []
    package_logger: logging.Logger = logging.getLogger("pdfplumber")
    handler = _CaptureHandler(log_records)
    old_propagate: bool = package_logger.propagate
    package_logger.addHandler(handler)
    package_logger.propagate = False

    record: dict[str, object] = {
        "id": case.identifier,
        "category": case.category,
        "callable": case.callable_name,
        "invocation": case.invocation,
    }
    try:
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            try:
                value: object = case.call(package, resources, phase)
            except Exception as error:  # noqa: BLE001 - exception is the contract
                record["outcome"] = {
                    "kind": "exception",
                    **_exception(error),
                }
            else:
                phase.mark("complete")
                record["outcome"] = {
                    "kind": "return",
                    "type": _type_name(type(value)),
                    "value": normalize(value),
                }
        record["warnings"] = [
            {
                "category": _type_name(item.category),
                "message": _stable_text(str(item.message)),
            }
            for item in caught
        ]
        record["logs"] = [
            {
                "level": item.levelname,
                "logger": item.name,
                "message": _stable_text(item.getMessage()),
            }
            for item in log_records
        ]
        record["phase"] = phase.current
    finally:
        package_logger.removeHandler(handler)
        package_logger.propagate = old_propagate
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


def _exception(error: BaseException) -> dict[str, object]:
    return {
        "type": _type_name(type(error)),
        "message": _stable_text(str(error)),
        "args": [_exception_arg(argument) for argument in error.args],
    }


def _exception_arg(value: object) -> object:
    if isinstance(value, BaseException):
        return _exception(value)
    return normalize(value)


def _type_name(value_type: type[object]) -> str:
    return f"{value_type.__module__}.{value_type.__qualname__}"


def _stable_text(value: str) -> str:
    return _MEMORY_ADDRESS.sub("<address>", value).replace(
        str(upstream.REPO_ROOT), "<repo>"
    )


def _module(package: ModuleType, suffix: str) -> ModuleType:
    return importlib.import_module(f"{package.__name__}.{suffix}")


def _empty_pdf(package: ModuleType, _: Resources, phase: Phase) -> object:
    phase.mark("open")
    return package.open(io.BytesIO(b""))


def _invalid_table_strategy(
    package: ModuleType, _: Resources, phase: Phase
) -> object:
    phase.mark("settings_validation")
    return _module(package, "table").TableSettings(vertical_strategy="bogus")


def _annotation(
    package: ModuleType,
    resources: Resources,
    phase: Phase,
    *,
    fatal: bool,
) -> object:
    phase.mark("open")
    with package.open(resources.annotation, raise_unicode_errors=fatal) as pdf:
        phase.mark("annotation_decode")
        annotations: list[dict[str, object]] = pdf.annots
        return {
            "annotation_count": len(annotations),
            "contents_present": "contents" in annotations[0],
            "contents_is_none": annotations[0].get("contents") is None,
        }


def _deprecated_vertical_ttb(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    phase.mark("open")
    with package.open(resources.basic) as pdf:
        phase.mark("word_extraction")
        return {"word_count": len(pdf.pages[0].extract_words(vertical_ttb=False))}


def _metadata(
    package: ModuleType,
    resources: Resources,
    phase: Phase,
    *,
    strict: bool,
) -> object:
    stream = io.BytesIO(resources.cyclic_metadata)
    phase.mark("metadata_decode")
    with package.open(stream, strict_metadata=strict) as pdf:
        return {
            "keys": sorted(pdf.metadata),
            "loop_value_type": _type_name(type(pdf.metadata["Loop"])),
            "external_stream_closed": stream.closed,
        }


def _password(
    package: ModuleType,
    resources: Resources,
    phase: Phase,
    password: str | None,
) -> object:
    phase.mark("password_authentication")
    with package.open(resources.password, password=password) as pdf:
        phase.mark("page_extraction")
        return {
            "page_count": len(pdf.pages),
            "first_page_char_count": len(pdf.pages[0].chars),
        }


class _RepairHelper:
    def __init__(self, name: str):
        self.name = name
        self._temporary: tempfile.TemporaryDirectory[str] | None = None

    def __enter__(self) -> Path:
        self._temporary = tempfile.TemporaryDirectory(prefix="pdfplumber-repair-")
        path = Path(self._temporary.name) / self.name
        path.write_text(REPAIR_HELPER_SOURCE, encoding="utf-8")
        path.chmod(0o755)
        return path

    def __exit__(self, *_: object) -> None:
        assert self._temporary is not None
        self._temporary.cleanup()


def _repair_bytesio(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    source = io.BytesIO(resources.basic.read_bytes())
    with _RepairHelper("repair-helper-prepress.py") as helper:
        phase.mark("repair_subprocess")
        repaired = package.repair(
            source,
            gs_path=helper,
            setting="prepress",
        )
        phase.mark("repair_result")
        payload: bytes = repaired.read()
        result = {
            "returned_type": _type_name(type(repaired)),
            "returned_sha256": hashlib.sha256(payload).hexdigest(),
            "source_closed": source.closed,
            "returned_closed": repaired.closed,
        }
        repaired.close()
        return result


def _repair_outfile(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    with (
        _RepairHelper("repair-helper.py") as helper,
        tempfile.TemporaryDirectory(prefix="pdfplumber-repair-output-") as directory,
    ):
        output = Path(directory) / "repaired.pdf"
        phase.mark("repair_subprocess")
        returned: object = package.repair(
            resources.basic,
            outfile=output,
            gs_path=helper,
        )
        phase.mark("repair_result")
        return {
            "return_is_none": returned is None,
            "outfile_exists": output.is_file(),
            "outfile_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
        }


def _repair_open(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    with _RepairHelper("repair-helper.py") as helper:
        phase.mark("repair_subprocess")
        with package.open(resources.basic, repair=True, gs_path=helper) as pdf:
            stream = pdf.stream
            phase.mark("page_extraction")
            result = {
                "page_count": len(pdf.pages),
                "path_is_none": pdf.path is None,
                "stream_is_external": pdf.stream_is_external,
                "stream_closed_in_context": stream.closed,
            }
        result["stream_closed_after_context"] = stream.closed
        return result


def _repair_failure(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    with _RepairHelper("repair-helper-fail.py") as helper:
        phase.mark("repair_subprocess")
        return package.repair(resources.basic, gs_path=helper)


def _owned_stream(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    phase.mark("open")
    pdf = package.open(resources.basic)
    first_page = pdf.pages[0]
    char_count: int = len(first_page.chars)
    stream = pdf.stream
    phase.mark("first_close")
    pdf.close()
    phase.mark("second_close")
    pdf.close()
    return {
        "stream_is_external": pdf.stream_is_external,
        "stream_closed": stream.closed,
        "page_count_after_close": len(pdf.pages),
        "first_page_identity_retained": pdf.pages[0] is first_page,
        "original_page_char_count_after_close": len(first_page.chars),
        "original_page_char_count_before_close": char_count,
    }


def _external_stream(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    stream = io.BytesIO(resources.basic.read_bytes())
    phase.mark("open")
    pdf = package.open(stream)
    page_count: int = len(pdf.pages)
    phase.mark("first_close")
    pdf.close()
    phase.mark("second_close")
    pdf.close()
    return {
        "stream_is_external": pdf.stream_is_external,
        "stream_closed": stream.closed,
        "page_count_before_close": page_count,
        "page_count_after_close": len(pdf.pages),
    }


def _closed_input(
    package: ModuleType, resources: Resources, phase: Phase
) -> object:
    stream = io.BytesIO(resources.basic.read_bytes())
    stream.close()
    phase.mark("open")
    return package.open(stream)


def _bbox(
    package: ModuleType,
    resources: Resources,
    phase: Phase,
    kind: str,
) -> object:
    phase.mark("open")
    with package.open(resources.basic) as pdf:
        page = pdf.pages[0]
        x0, top, x1, bottom = page.bbox
        boxes: dict[str, tuple[float, float, float, float]] = {
            "zero": (x0, top, x0, top),
            "negative_width": (x0 + 10, top, x0, top + 10),
            "negative_height": (x0, top + 10, x0 + 10, top),
            "outside": (x1 + 1, bottom + 1, x1 + 11, bottom + 11),
            "partial": (x0 - 1, top, x0 + 10, top + 10),
            "strict_false": (x0 - 10, top - 10, x1 + 10, bottom + 10),
        }
        phase.mark("bbox_validation")
        cropped = page.crop(boxes[kind], strict=kind != "strict_false")
        return {"bbox": cropped.bbox, "width": cropped.width, "height": cropped.height}


def cases() -> tuple[Case, ...]:
    records = [
        Case(
            "bbox.entirely_outside",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$entirely_outside", "strict": True},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "outside"
            ),
        ),
        Case(
            "bbox.negative_height",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$negative_height", "strict": True},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "negative_height"
            ),
        ),
        Case(
            "bbox.negative_width",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$negative_width", "strict": True},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "negative_width"
            ),
        ),
        Case(
            "bbox.partially_outside",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$partially_outside", "strict": True},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "partial"
            ),
        ),
        Case(
            "bbox.strict_false",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$outside_parent", "strict": False},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "strict_false"
            ),
        ),
        Case(
            "bbox.zero_area",
            "invalid_bounding_boxes",
            "pdfplumber.page.Page.crop",
            {"bbox": "$zero_area", "strict": True},
            "open",
            lambda package, resources, phase: _bbox(
                package, resources, phase, "zero"
            ),
        ),
        Case(
            "closed.external_stream",
            "closed_resources",
            "pdfplumber.pdf.PDF.close",
            {"input": "external BytesIO", "close_calls": 2},
            "open",
            _external_stream,
        ),
        Case(
            "closed.initial_input",
            "closed_resources",
            "pdfplumber.open",
            {"input": "closed BytesIO"},
            "open",
            _closed_input,
        ),
        Case(
            "closed.owned_stream",
            "closed_resources",
            "pdfplumber.pdf.PDF.close",
            {"input": BASIC_FIXTURE.as_posix(), "close_calls": 2},
            "open",
            _owned_stream,
        ),
        Case(
            "exception.empty_pdf",
            "exceptions",
            "pdfplumber.open",
            {"input": {"bytes_hex": ""}},
            "open",
            _empty_pdf,
        ),
        Case(
            "exception.table_settings_strategy",
            "exceptions",
            "pdfplumber.table.TableSettings",
            {"vertical_strategy": "bogus"},
            "settings_validation",
            _invalid_table_strategy,
        ),
        Case(
            "metadata.non_strict_cycle",
            "malformed_metadata",
            "pdfplumber.open",
            {"input": "inline:cyclic-metadata.pdf", "strict_metadata": False},
            "metadata_decode",
            lambda package, resources, phase: _metadata(
                package, resources, phase, strict=False
            ),
        ),
        Case(
            "metadata.strict_cycle",
            "malformed_metadata",
            "pdfplumber.open",
            {"input": "inline:cyclic-metadata.pdf", "strict_metadata": True},
            "metadata_decode",
            lambda package, resources, phase: _metadata(
                package, resources, phase, strict=True
            ),
        ),
        Case(
            "password.correct",
            "passwords",
            "pdfplumber.open",
            {"input": PASSWORD_FIXTURE.as_posix(), "password": "<redacted>"},
            "password_authentication",
            lambda package, resources, phase: _password(
                package, resources, phase, "test"
            ),
        ),
        Case(
            "password.missing",
            "passwords",
            "pdfplumber.open",
            {"input": PASSWORD_FIXTURE.as_posix(), "password": None},
            "password_authentication",
            lambda package, resources, phase: _password(
                package, resources, phase, None
            ),
        ),
        Case(
            "password.wrong",
            "passwords",
            "pdfplumber.open",
            {"input": PASSWORD_FIXTURE.as_posix(), "password": "<redacted>"},
            "password_authentication",
            lambda package, resources, phase: _password(
                package, resources, phase, "wrong"
            ),
        ),
        Case(
            "repair.bytesio_return",
            "repair",
            "pdfplumber.repair",
            {"input": "BytesIO", "setting": "prepress"},
            "repair_subprocess",
            _repair_bytesio,
        ),
        Case(
            "repair.open_internal_stream",
            "repair",
            "pdfplumber.open",
            {"input": BASIC_FIXTURE.as_posix(), "repair": True},
            "repair_subprocess",
            _repair_open,
        ),
        Case(
            "repair.outfile",
            "repair",
            "pdfplumber.repair",
            {"input": BASIC_FIXTURE.as_posix(), "outfile": "<temporary>"},
            "repair_subprocess",
            _repair_outfile,
        ),
        Case(
            "repair.process_failure",
            "repair",
            "pdfplumber.repair",
            {"input": BASIC_FIXTURE.as_posix(), "helper_exit": 7},
            "repair_subprocess",
            _repair_failure,
        ),
        Case(
            "warning.annotation_unicode_fatal",
            "warnings",
            "pdfplumber.page.Page.annots",
            {
                "input": ANNOTATION_FIXTURE.as_posix(),
                "raise_unicode_errors": True,
            },
            "open",
            lambda package, resources, phase: _annotation(
                package, resources, phase, fatal=True
            ),
        ),
        Case(
            "warning.annotation_unicode_nonfatal",
            "warnings",
            "pdfplumber.page.Page.annots",
            {
                "input": ANNOTATION_FIXTURE.as_posix(),
                "raise_unicode_errors": False,
            },
            "open",
            lambda package, resources, phase: _annotation(
                package, resources, phase, fatal=False
            ),
        ),
        Case(
            "warning.deprecated_vertical_ttb",
            "warnings",
            "pdfplumber.page.Page.extract_words",
            {"input": BASIC_FIXTURE.as_posix(), "vertical_ttb": False},
            "open",
            _deprecated_vertical_ttb,
        ),
    ]
    return tuple(sorted(records, key=lambda case: case.identifier))
