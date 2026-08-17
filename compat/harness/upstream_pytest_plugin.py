"""Pytest result recorder loaded inside the isolated candidate environment."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any


_collected: set[str] = set()
_failed: set[str] = set()


def pytest_sessionstart(session: Any) -> None:
    import pdfplumber

    expected = Path(os.environ["PDFPLUMBER_EXPECTED_CANDIDATE_ORIGIN"]).resolve()
    actual = Path(pdfplumber.__file__).resolve()
    if actual != expected:
        raise RuntimeError(
            f"candidate import changed after preflight: expected {expected}, got {actual}"
        )


def pytest_collection_modifyitems(items: list[Any]) -> None:
    _collected.update(item.nodeid for item in items)


def pytest_xdist_node_collection_finished(node: Any, ids: list[str]) -> None:
    _collected.update(ids)


def pytest_runtest_logreport(report: Any) -> None:
    if report.failed:
        _failed.add(report.nodeid)


def pytest_sessionfinish(session: Any, exitstatus: int) -> None:
    if hasattr(session.config, "workerinput"):
        return
    output = Path(os.environ["PDFPLUMBER_UPSTREAM_RESULTS"])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "pytest_exit_code": int(exitstatus),
                "collected": sorted(_collected),
                "failed": sorted(_failed),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
