"""JSON value conversion compatible with the pinned pdfplumber release."""

from __future__ import annotations

import base64
import csv
from collections.abc import Callable
from io import StringIO
from typing import Any

CSV_COLS_REQUIRED = ["object_type"]
CSV_COLS_TO_PREPEND = [
    "page_number",
    "x0",
    "x1",
    "y0",
    "y1",
    "doctop",
    "top",
    "bottom",
    "width",
    "height",
]
ENCODINGS_TO_TRY = ["utf-8", "latin-1", "utf-16", "utf-16le"]
_PDFDOC_CODEPOINT_OVERRIDES = {
    0x16: 0x0017,
    0x17: 0x0017,
    0x18: 0x02D8,
    0x19: 0x02C7,
    0x1A: 0x02C6,
    0x1B: 0x02D9,
    0x1C: 0x02DD,
    0x1D: 0x02DB,
    0x1E: 0x02DA,
    0x1F: 0x02DC,
    0x7F: 0,
    0x80: 0x2022,
    0x81: 0x2020,
    0x82: 0x2021,
    0x83: 0x2026,
    0x84: 0x2014,
    0x85: 0x2013,
    0x86: 0x0192,
    0x87: 0x2044,
    0x88: 0x2039,
    0x89: 0x203A,
    0x8A: 0x2212,
    0x8B: 0x2030,
    0x8C: 0x201E,
    0x8D: 0x201C,
    0x8E: 0x201D,
    0x8F: 0x2018,
    0x90: 0x2019,
    0x91: 0x201A,
    0x92: 0x2122,
    0x93: 0xFB01,
    0x94: 0xFB02,
    0x95: 0x0141,
    0x96: 0x0152,
    0x97: 0x0160,
    0x98: 0x0178,
    0x99: 0x017D,
    0x9A: 0x0131,
    0x9B: 0x0142,
    0x9C: 0x0153,
    0x9D: 0x0161,
    0x9E: 0x017E,
    0x9F: 0,
    0xA0: 0x20AC,
    0xAD: 0,
}
_PDFDOC_ENCODING = tuple(
    chr(_PDFDOC_CODEPOINT_OVERRIDES.get(value, value)) for value in range(256)
)


def _decode_pdf_text(value: bytes | str) -> str:
    if isinstance(value, bytes) and value.startswith(b"\xfe\xff"):
        return str(value[2:], "utf-16be", "ignore")
    try:
        indices = (
            ord(character) if isinstance(character, str) else character
            for character in value
        )
        return "".join(_PDFDOC_ENCODING[index] for index in indices)
    except IndexError:
        return str(value)


def get_attr_filter(
    include_attrs: list[str] | None = None,
    exclude_attrs: list[str] | None = None,
) -> Callable[[str], bool]:
    if include_attrs is not None and exclude_attrs is not None:
        raise ValueError(
            "Cannot specify `include_attrs` and `exclude_attrs` at the same time."
        )
    if include_attrs is not None:
        included = set(CSV_COLS_REQUIRED + include_attrs)
        return lambda attr: attr in included
    if exclude_attrs is not None:
        nonexcludable = set(exclude_attrs).intersection(set(CSV_COLS_REQUIRED))
        if nonexcludable:
            raise ValueError(
                f"Cannot exclude these required properties: {list(nonexcludable)}"
            )
        excluded = set(exclude_attrs)
        return lambda attr: attr not in excluded
    return lambda attr: True


class Serializer:
    def __init__(
        self,
        precision: int | None = None,
        include_attrs: list[str] | None = None,
        exclude_attrs: list[str] | None = None,
    ) -> None:
        self.precision = precision
        self.attr_filter = get_attr_filter(include_attrs, exclude_attrs)

    def serialize(self, obj: Any) -> Any:
        if obj is None:
            return None
        object_type = type(obj)
        if object_type in (int, str):
            return obj
        converter = getattr(self, f"do_{object_type.__name__}", None)
        return converter(obj) if converter is not None else str(obj)

    def do_float(self, value: float) -> float:
        return value if self.precision is None else round(value, self.precision)

    def do_bool(self, value: bool) -> int:
        return int(value)

    def do_list(self, value: list[Any]) -> list[Any]:
        return [self.serialize(item) for item in value]

    def do_tuple(self, value: tuple[Any, ...]) -> tuple[Any, ...]:
        return tuple(self.serialize(item) for item in value)

    def do_dict(self, value: dict[str, Any]) -> dict[str, Any]:
        if "object_type" in value:
            return {
                key: self.serialize(item)
                for key, item in value.items()
                if self.attr_filter(key)
            }
        return {key: self.serialize(item) for key, item in value.items()}

    def do_PDFStream(self, value: Any) -> dict[str, str | None]:
        rawdata = value.rawdata
        return {
            "rawdata": (
                base64.b64encode(rawdata).decode("ascii") if rawdata else None
            )
        }

    def do_PSLiteral(self, value: Any) -> str:
        return _decode_pdf_text(value.name)

    def do_bytes(self, value: bytes) -> str | None:
        for encoding in ENCODINGS_TO_TRY:
            try:
                return value.decode(encoding)
            except UnicodeDecodeError:
                return None
        value.decode(ENCODINGS_TO_TRY[0])
        return None


def serialize_csv(
    pages: list[dict[str, Any]],
    stream: Any = None,
    precision: int | None = None,
    include_attrs: list[str] | None = None,
    exclude_attrs: list[str] | None = None,
) -> str | None:
    if stream is None:
        stream = StringIO()
        to_string = True
    else:
        to_string = False

    serialized: list[dict[str, Any]] = []
    fields: set[str] = set()
    serializer = Serializer(
        precision=precision,
        include_attrs=include_attrs,
        exclude_attrs=exclude_attrs,
    )
    for page in pages:
        object_lists = (
            value
            for key, value in page.items()
            if key in {"chars", "lines", "rects", "curves", "images", "annots"}
        )
        for objects in object_lists:
            if len(objects):
                rows = []
                for item in objects:
                    row = dict(item)
                    row.setdefault("page_number", page["page_number"])
                    rows.append(row)
                serialized += serializer.serialize(rows)
                new_keys = [
                    key
                    for key, value in objects[0].items()
                    if type(value) is not dict
                ]
                fields = fields.union(set(new_keys))

    non_required = CSV_COLS_TO_PREPEND + list(
        sorted(set(fields) - set(CSV_COLS_REQUIRED + CSV_COLS_TO_PREPEND))
    )
    columns = CSV_COLS_REQUIRED + list(
        filter(serializer.attr_filter, non_required)
    )
    writer = csv.DictWriter(
        stream,
        fieldnames=columns,
        extrasaction="ignore",
        quoting=csv.QUOTE_MINIMAL,
        escapechar="\\",
    )
    writer.writeheader()
    writer.writerows(serialized)

    if to_string:
        stream.seek(0)
        return stream.read()
    return None
