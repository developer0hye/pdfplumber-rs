"""JSON value conversion compatible with the pinned pdfplumber release."""

from __future__ import annotations

import base64
from collections.abc import Callable
from typing import Any

REQUIRED_OBJECT_ATTRS = ["object_type"]
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
        included = set(REQUIRED_OBJECT_ATTRS + include_attrs)
        return lambda attr: attr in included
    if exclude_attrs is not None:
        nonexcludable = set(exclude_attrs).intersection(set(REQUIRED_OBJECT_ATTRS))
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
