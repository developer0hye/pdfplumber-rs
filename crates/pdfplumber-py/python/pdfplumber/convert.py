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
