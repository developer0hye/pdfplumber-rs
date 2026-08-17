"""Deterministic Python public-API reflection for PARITY-005.

The snapshot records what an ordinary Python caller can import and inspect. It
does not use a hand-maintained symbol list: modules are discovered recursively,
module exports follow Python's public-name and ``__all__`` rules, and class
members include inherited public descriptors.
"""

from __future__ import annotations

import functools
import importlib
import inspect
import math
import pkgutil
import re
import types
from collections.abc import ItemsView, KeysView, Mapping, Sequence, Set, ValuesView
from pathlib import Path
from typing import Any

from compat.harness import lockfile, upstream


SCHEMA_VERSION: int = 1
PACKAGE_NAME: str = "pdfplumber"
PUBLIC_PROTOCOL_METHODS: frozenset[str] = frozenset(
    {
        "__bool__",
        "__call__",
        "__contains__",
        "__enter__",
        "__exit__",
        "__getitem__",
        "__init__",
        "__iter__",
        "__len__",
        "__repr__",
        "__str__",
    }
)

_MEMORY_ADDRESS: re.Pattern[str] = re.compile(r"0x[0-9a-fA-F]+")
_MAX_VALUE_DEPTH: int = 8


def snapshot_path() -> Path:
    target: upstream.Target = upstream.load_target()
    return (
        upstream.REPO_ROOT
        / "compat"
        / "snapshots"
        / f"{target.project}-v{target.version}-api.json"
    )


def build(root_package: types.ModuleType) -> dict[str, object]:
    """Reflect a complete importable package tree into JSON-safe data."""
    target: upstream.Target = upstream.load_target()
    environment: upstream.Environment = upstream.load_environment()
    modules: dict[str, dict[str, object]] = {}
    for module_name in discover_module_names(root_package):
        module: types.ModuleType = importlib.import_module(module_name)
        modules[module_name] = snapshot_module(module)

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
        "modules": modules,
    }


def discover_module_names(root_package: types.ModuleType) -> list[str]:
    names: set[str] = {root_package.__name__}
    package_path: object = getattr(root_package, "__path__", None)
    if package_path is None:
        return sorted(names)

    prefix: str = f"{root_package.__name__}."
    for module_info in pkgutil.walk_packages(package_path, prefix):
        names.add(module_info.name)
    return sorted(names)


def snapshot_module(module: types.ModuleType) -> dict[str, object]:
    declared_all_raw: object = getattr(module, "__all__", None)
    declared_all: list[str] | None
    if declared_all_raw is None:
        declared_all = None
    else:
        declared_all = [str(name) for name in declared_all_raw]  # type: ignore[union-attr]

    export_names: set[str] = {
        name for name in vars(module) if not name.startswith("_")
    }
    if declared_all is not None:
        export_names.update(declared_all)

    exports: dict[str, dict[str, object]] = {}
    for name in sorted(export_names):
        if not hasattr(module, name):
            exports[name] = {"kind": "missing", "declared_in_all": True}
            continue
        exports[name] = snapshot_export(getattr(module, name), module.__name__)

    is_package: bool = hasattr(module, "__path__")
    return {
        "public": _is_public_module(module.__name__),
        "is_package": is_package,
        "source": _logical_source_path(module.__name__, is_package),
        "all": declared_all,
        "exports": exports,
    }


def snapshot_export(value: object, exporting_module: str) -> dict[str, object]:
    if isinstance(value, types.ModuleType):
        return {"kind": "module", "module": value.__name__}

    defined_in: str | None = _defined_in(value)
    qualname: str | None = _qualname(value)

    if inspect.isclass(value):
        result: dict[str, object] = {
            "kind": "class",
            "defined_in": defined_in,
            "qualname": qualname,
            "signature": signature(value),
            "bases": [_type_name(base) for base in value.__bases__],
            "mro": [_type_name(base) for base in value.__mro__],
        }
        if defined_in == exporting_module:
            result["members"] = snapshot_class_members(value)
        return result

    if inspect.ismethod(value):
        return {
            "kind": "method",
            "defined_in": defined_in,
            "qualname": qualname,
            "signature": signature(value),
        }

    if inspect.isfunction(value):
        return {
            "kind": "function",
            "defined_in": defined_in,
            "qualname": qualname,
            "signature": signature(value),
        }

    if inspect.isbuiltin(value):
        return {
            "kind": "builtin",
            "defined_in": defined_in,
            "qualname": qualname,
            "signature": signature(value),
        }

    if callable(value):
        return {
            "kind": "callable",
            "defined_in": defined_in,
            "type": _type_name(type(value)),
            "signature": signature(value),
        }

    return {
        "kind": "constant",
        "defined_in": defined_in,
        "type": _type_name(type(value)),
        "value": normalize_value(value),
    }


def snapshot_class_members(cls: type[object]) -> dict[str, dict[str, object]]:
    members: dict[str, dict[str, object]] = {}
    for name, raw_value in inspect.getmembers_static(cls):
        defining_class: type[object] | None = _defining_class(cls, name)
        defining_module: str = (
            defining_class.__module__ if defining_class is not None else "builtins"
        )
        is_public_name: bool = not name.startswith("_")
        is_package_protocol: bool = (
            name in PUBLIC_PROTOCOL_METHODS
            and defining_module.startswith(PACKAGE_NAME)
        )
        if not is_public_name and not is_package_protocol:
            continue
        members[name] = snapshot_class_member(cls, name, raw_value, defining_class)
    return members


def snapshot_class_member(
    cls: type[object],
    name: str,
    raw_value: object,
    defining_class: type[object] | None,
) -> dict[str, object]:
    defined_in: str | None = (
        _type_name(defining_class) if defining_class is not None else None
    )

    if isinstance(raw_value, classmethod):
        return {
            "kind": "classmethod",
            "defined_in": defined_in,
            "signature": signature(getattr(cls, name)),
        }
    if isinstance(raw_value, staticmethod):
        return {
            "kind": "staticmethod",
            "defined_in": defined_in,
            "signature": signature(getattr(cls, name)),
        }
    if isinstance(raw_value, property):
        return {
            "kind": "property",
            "defined_in": defined_in,
            "signature": signature(raw_value.fget),
            "readable": raw_value.fget is not None,
            "writable": raw_value.fset is not None,
            "deletable": raw_value.fdel is not None,
        }
    if isinstance(raw_value, functools.cached_property):
        return {
            "kind": "cached_property",
            "defined_in": defined_in,
            "signature": signature(raw_value.func),
        }
    if inspect.isfunction(raw_value):
        return {
            "kind": "method",
            "defined_in": defined_in,
            "signature": signature(raw_value),
        }
    if inspect.ismethoddescriptor(raw_value):
        return {
            "kind": "method_descriptor",
            "defined_in": defined_in,
            "signature": signature(raw_value),
        }
    if inspect.isdatadescriptor(raw_value):
        return {
            "kind": "data_descriptor",
            "defined_in": defined_in,
            "type": _type_name(type(raw_value)),
        }
    if inspect.isclass(raw_value):
        return {
            "kind": "class",
            "defined_in": _defined_in(raw_value),
            "qualname": _qualname(raw_value),
            "signature": signature(raw_value),
        }
    if callable(raw_value):
        return {
            "kind": "callable",
            "defined_in": defined_in,
            "type": _type_name(type(raw_value)),
            "signature": signature(raw_value),
        }
    return {
        "kind": "constant",
        "defined_in": defined_in,
        "type": _type_name(type(raw_value)),
        "value": normalize_value(raw_value),
    }


def signature(value: object) -> str | None:
    try:
        return _stable_signature(inspect.signature(value, eval_str=False))
    except (TypeError, ValueError):
        if not inspect.isclass(value):
            return None

    try:
        initializer: inspect.Signature = inspect.signature(
            value.__init__, eval_str=False  # type: ignore[misc]
        )
    except (AttributeError, TypeError, ValueError):
        return None

    parameters: list[inspect.Parameter] = list(initializer.parameters.values())
    if parameters and parameters[0].name in {"self", "cls"}:
        parameters = parameters[1:]
    return _stable_signature(initializer.replace(parameters=parameters))


def normalize_value(
    value: object,
    *,
    _depth: int = 0,
    _seen: frozenset[int] = frozenset(),
) -> object:
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        if math.isnan(value):
            return {"float": "nan"}
        if math.isinf(value):
            return {"float": "infinity" if value > 0 else "-infinity"}
        return value
    if isinstance(value, bytes):
        return {"bytes_hex": value.hex()}
    if _depth >= _MAX_VALUE_DEPTH:
        return {"type": _type_name(type(value)), "repr": "<maximum-depth>"}

    identity: int = id(value)
    if identity in _seen:
        return {"type": _type_name(type(value)), "repr": "<cycle>"}
    seen: frozenset[int] = _seen | {identity}

    if isinstance(value, Mapping):
        entries: list[list[object]] = [
            [
                normalize_value(key, _depth=_depth + 1, _seen=seen),
                normalize_value(item, _depth=_depth + 1, _seen=seen),
            ]
            for key, item in value.items()
        ]
        entries.sort(key=lambda entry: repr(entry[0]))
        return {"mapping": entries}
    if isinstance(value, KeysView):
        return {
            "keys": [
                normalize_value(item, _depth=_depth + 1, _seen=seen)
                for item in value
            ]
        }
    if isinstance(value, ValuesView):
        return {
            "values": [
                normalize_value(item, _depth=_depth + 1, _seen=seen)
                for item in value
            ]
        }
    if isinstance(value, ItemsView):
        return {
            "items": [
                normalize_value(item, _depth=_depth + 1, _seen=seen)
                for item in value
            ]
        }
    if isinstance(value, tuple):
        return {
            "tuple": [
                normalize_value(item, _depth=_depth + 1, _seen=seen)
                for item in value
            ]
        }
    if isinstance(value, Sequence):
        return [
            normalize_value(item, _depth=_depth + 1, _seen=seen) for item in value
        ]
    if isinstance(value, Set):
        items: list[object] = [
            normalize_value(item, _depth=_depth + 1, _seen=seen) for item in value
        ]
        items.sort(key=repr)
        return {"set": items}

    stable_repr: str = _MEMORY_ADDRESS.sub("<address>", repr(value))
    return {"type": _type_name(type(value)), "repr": stable_repr}


def _is_public_module(module_name: str) -> bool:
    return all(not component.startswith("_") for component in module_name.split(".")[1:])


def _stable_signature(value: inspect.Signature) -> str:
    return _MEMORY_ADDRESS.sub("<address>", str(value))


def _logical_source_path(module_name: str, is_package: bool) -> str:
    base: str = module_name.replace(".", "/")
    return f"{base}/__init__.py" if is_package else f"{base}.py"


def _defined_in(value: object) -> str | None:
    module_name: object = getattr(value, "__module__", None)
    if isinstance(module_name, str):
        return module_name
    type_module: str = type(value).__module__
    return type_module if type_module else None


def _qualname(value: object) -> str | None:
    name: object = getattr(value, "__qualname__", None)
    return name if isinstance(name, str) else None


def _type_name(value_type: type[object] | None) -> str:
    if value_type is None:
        return ""
    return f"{value_type.__module__}.{value_type.__qualname__}"


def _defining_class(cls: type[object], name: str) -> type[object] | None:
    for base in cls.__mro__:
        if name in vars(base):
            return base
    return None
