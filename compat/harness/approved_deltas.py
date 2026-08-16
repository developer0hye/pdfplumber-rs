"""Exact intentional-difference registry and gate (PARITY-016)."""

from __future__ import annotations

import base64
import hashlib
import json
import math
import re
import tomllib
from dataclasses import dataclass
from decimal import Decimal
from enum import Enum
from pathlib import Path
from typing import Any


_IDENTIFIER_PATTERN: re.Pattern[str] = re.compile(r"^DELTA-[0-9]{3,}$")
_SHA256_PATTERN: re.Pattern[str] = re.compile(r"^[0-9a-f]{64}$")


class DeltaRegistryError(RuntimeError):
    """The approved-delta registry or an observed value is not exact."""


@dataclass(frozen=True, order=True)
class ObservedDelta:
    fixture: str
    page: int
    api: str
    upstream_sha256: str
    rust_sha256: str

    @property
    def match_key(self) -> tuple[str, int, str, str, str]:
        return (
            self.fixture,
            self.page,
            self.api,
            self.upstream_sha256,
            self.rust_sha256,
        )


@dataclass(frozen=True, order=True)
class ApprovedDelta:
    identifier: str
    fixture: str
    page: int
    api: str
    upstream_result: str
    upstream_sha256: str
    rust_result: str
    rust_sha256: str
    technical_reason: str
    compatibility_risk: str
    approving_maintainer: str
    regression_test: str
    review_condition: str

    @property
    def match_key(self) -> tuple[str, int, str, str, str]:
        return (
            self.fixture,
            self.page,
            self.api,
            self.upstream_sha256,
            self.rust_sha256,
        )


@dataclass(frozen=True)
class Registry:
    version: str
    commit: str
    deltas: tuple[ApprovedDelta, ...]


@dataclass(frozen=True)
class GateResult:
    approved: tuple[tuple[ObservedDelta, ApprovedDelta], ...]
    unregistered: tuple[ObservedDelta, ...]
    stale: tuple[ApprovedDelta, ...]
    exit_code: int


def validate_target(registry: Registry, version: str, commit: str) -> None:
    if (registry.version, registry.commit) != (version, commit):
        raise DeltaRegistryError(
            "approved-delta registry target "
            f"{registry.version}/{registry.commit} does not match "
            f"{version}/{commit}"
        )


def load_registry(path: Path) -> Registry:
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise DeltaRegistryError(f"cannot read approved-delta registry: {path}") from error
    if data.get("schema_version") != 1:
        raise DeltaRegistryError(f"unsupported approved-delta schema in {path}")
    target = _table(data, "target", path)
    raw_deltas = data.get("delta", [])
    if not isinstance(raw_deltas, list):
        raise DeltaRegistryError(f"delta must be an array of tables in {path}")

    deltas: list[ApprovedDelta] = []
    identifiers: set[str] = set()
    match_keys: set[tuple[str, int, str, str, str]] = set()
    for index, raw in enumerate(raw_deltas):
        if not isinstance(raw, dict):
            raise DeltaRegistryError(f"delta[{index}] must be a table in {path}")
        delta = _load_delta(raw, path, index)
        if delta.identifier in identifiers:
            raise DeltaRegistryError(
                f"duplicate approved-delta identifier: {delta.identifier}"
            )
        if delta.match_key in match_keys:
            raise DeltaRegistryError(
                f"duplicate approved-delta match for {delta.fixture} page "
                f"{delta.page} {delta.api}"
            )
        identifiers.add(delta.identifier)
        match_keys.add(delta.match_key)
        deltas.append(delta)
    return Registry(
        version=_string(target, "version", path),
        commit=_string(target, "commit", path),
        deltas=tuple(sorted(deltas)),
    )


def evaluate(
    observed: tuple[ObservedDelta, ...],
    registry: Registry,
) -> GateResult:
    observed_by_key: dict[tuple[str, int, str, str, str], ObservedDelta] = {}
    for delta in observed:
        _validate_observation(delta)
        if delta.match_key in observed_by_key:
            raise DeltaRegistryError(
                f"duplicate observed delta for {delta.fixture} page "
                f"{delta.page} {delta.api}"
            )
        observed_by_key[delta.match_key] = delta
    registered_by_key = {delta.match_key: delta for delta in registry.deltas}

    approved = tuple(
        sorted(
            (delta, registered_by_key[delta.match_key])
            for delta in observed
            if delta.match_key in registered_by_key
        )
    )
    unregistered = tuple(
        sorted(
            delta for delta in observed if delta.match_key not in registered_by_key
        )
    )
    stale = tuple(
        sorted(
            delta
            for delta in registry.deltas
            if delta.match_key not in observed_by_key
        )
    )
    return GateResult(
        approved=approved,
        unregistered=unregistered,
        stale=stale,
        exit_code=int(bool(unregistered or stale)),
    )


def value_digest(value: Any) -> str:
    encoded = json.dumps(
        _canonical_value(value),
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _canonical_value(value: Any) -> list[Any]:
    if value is None:
        return ["none"]
    if isinstance(value, bool):
        return ["bool", value]
    if isinstance(value, int):
        return ["int", str(value)]
    if isinstance(value, float):
        if math.isnan(value):
            encoded = "nan"
        elif math.isinf(value):
            encoded = "inf" if value > 0 else "-inf"
        else:
            encoded = value.hex()
        return ["float", encoded]
    if isinstance(value, Decimal):
        return ["decimal", str(value)]
    if isinstance(value, str):
        return ["str", value]
    if isinstance(value, bytes):
        return ["bytes", base64.b64encode(value).decode("ascii")]
    if isinstance(value, list):
        return ["list", [_canonical_value(item) for item in value]]
    if isinstance(value, tuple):
        return ["tuple", [_canonical_value(item) for item in value]]
    if isinstance(value, dict):
        items = [
            [_canonical_value(key), _canonical_value(nested)]
            for key, nested in value.items()
        ]
        items.sort(
            key=lambda item: json.dumps(
                item[0],
                ensure_ascii=False,
                separators=(",", ":"),
            )
        )
        return ["dict", items]
    value_type = type(value)
    type_name = f"{value_type.__module__}.{value_type.__qualname__}"
    if type_name in {
        "pdfminer.psparser.PSKeyword",
        "pdfminer.psparser.PSLiteral",
    }:
        return [type_name, _canonical_value(_required_attribute(value, "name"))]
    if type_name == "pdfminer.pdftypes.PDFObjRef":
        return [type_name, _canonical_value(_required_attribute(value, "objid"))]
    if type_name == "pdfminer.pdftypes.PDFStream":
        return [
            type_name,
            [
                "attrs",
                _canonical_value(_required_attribute(value, "attrs")),
            ],
            [
                "rawdata",
                _canonical_value(_required_attribute(value, "rawdata")),
            ],
            ["objid", _canonical_value(_required_attribute(value, "objid"))],
            ["genno", _canonical_value(_required_attribute(value, "genno"))],
        ]
    if type_name == "pdfplumber.page.Page":
        attributes = (
            "page_number",
            "initial_doctop",
            "rotation",
            "mediabox",
            "cropbox",
            "bbox",
        )
        return [
            type_name,
            [
                [name, _canonical_value(_required_attribute(value, name))]
                for name in attributes
            ],
        ]
    if isinstance(value, Enum):
        return [
            "enum",
            type_name,
            value.name,
        ]
    raise DeltaRegistryError(
        "cannot create an exact approved-delta digest for "
        f"{type(value).__module__}.{type(value).__qualname__}"
    )


def _required_attribute(value: Any, name: str) -> Any:
    try:
        return getattr(value, name)
    except AttributeError as error:
        raise DeltaRegistryError(
            f"{type(value).__module__}.{type(value).__qualname__} "
            f"has no stable {name} attribute"
        ) from error


def _load_delta(raw: dict[str, object], path: Path, index: int) -> ApprovedDelta:
    identifier = _string(raw, "id", path)
    if _IDENTIFIER_PATTERN.fullmatch(identifier) is None:
        raise DeltaRegistryError(f"invalid approved-delta identifier: {identifier}")
    fixture = _string(raw, "fixture", path)
    fixture_path = Path(fixture)
    if fixture_path.is_absolute() or ".." in fixture_path.parts:
        raise DeltaRegistryError(f"delta[{index}] has unsafe fixture path: {fixture}")
    page = raw.get("page")
    if isinstance(page, bool) or not isinstance(page, int) or page < 1:
        raise DeltaRegistryError(f"delta[{index}].page must be a positive integer")
    upstream_sha256 = _sha256(raw, "upstream_sha256", path)
    rust_sha256 = _sha256(raw, "rust_sha256", path)
    return ApprovedDelta(
        identifier=identifier,
        fixture=fixture_path.as_posix(),
        page=page,
        api=_string(raw, "api", path),
        upstream_result=_string(raw, "upstream_result", path),
        upstream_sha256=upstream_sha256,
        rust_result=_string(raw, "rust_result", path),
        rust_sha256=rust_sha256,
        technical_reason=_string(raw, "technical_reason", path),
        compatibility_risk=_string(raw, "compatibility_risk", path),
        approving_maintainer=_string(raw, "approving_maintainer", path),
        regression_test=_string(raw, "regression_test", path),
        review_condition=_string(raw, "review_condition", path),
    )


def _validate_observation(delta: ObservedDelta) -> None:
    fixture = Path(delta.fixture)
    if fixture.is_absolute() or ".." in fixture.parts:
        raise DeltaRegistryError(f"unsafe observed fixture path: {delta.fixture}")
    if delta.page < 1:
        raise DeltaRegistryError("observed delta page must be positive")
    if not delta.api:
        raise DeltaRegistryError("observed delta API must not be empty")
    for digest in (delta.upstream_sha256, delta.rust_sha256):
        if _SHA256_PATTERN.fullmatch(digest) is None:
            raise DeltaRegistryError(f"invalid observed result SHA-256: {digest}")


def _sha256(data: dict[str, object], key: str, path: Path) -> str:
    value = _string(data, key, path)
    if _SHA256_PATTERN.fullmatch(value) is None:
        raise DeltaRegistryError(f"{key} must be a SHA-256 digest in {path}")
    return value


def _table(
    data: dict[str, object],
    key: str,
    path: Path,
) -> dict[str, object]:
    value = data.get(key)
    if not isinstance(value, dict):
        raise DeltaRegistryError(f"{key} must be a table in {path}")
    return value


def _string(data: dict[str, object], key: str, path: Path) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise DeltaRegistryError(f"{key} must be a non-empty string in {path}")
    return value
