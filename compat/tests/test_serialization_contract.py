"""Exact JSON/CSV serialization differential contract (PARITY-013)."""

from __future__ import annotations

import csv
import hashlib
import io
import json
import unittest
from pathlib import Path

from compat.harness import lockfile, serialization_contract, upstream


CONTRACT_PATH: Path = (
    upstream.REPO_ROOT
    / "compat"
    / "contracts"
    / "pdfplumber-v0.11.10-serialization.json"
)
FIXTURE: str = "crates/pdfplumber/tests/fixtures/pdfs/issue-67-example.pdf"
BASIC_FIXTURE: str = "tests/fixtures/generated/basic_text.pdf"
EXPECTED_CASES: frozenset[str] = frozenset(
    {
        "csv.invalid_both_filters",
        "csv.invalid_required_exclude",
        "csv.page_default",
        "csv.page_stream",
        "csv.pdf_basic_text_default",
        "csv.pdf_chars_only",
        "csv.pdf_default",
        "csv.pdf_exclude",
        "csv.pdf_include",
        "csv.pdf_no_objects",
        "csv.pdf_precision_0",
        "csv.pdf_precision_3",
        "csv.pdf_stream",
        "json.invalid_both_filters",
        "json.invalid_required_exclude",
        "json.page_default",
        "json.page_precision_3",
        "json.page_stream",
        "json.pdf_basic_text_default",
        "json.pdf_chars_only",
        "json.pdf_default",
        "json.pdf_exclude",
        "json.pdf_include",
        "json.pdf_indent_2",
        "json.pdf_no_objects",
        "json.pdf_precision_0",
        "json.pdf_precision_3",
        "json.pdf_stream",
    }
)


class SerializationContractTests(unittest.TestCase):
    def test_catalog_covers_exact_json_csv_surfaces_and_options(self) -> None:
        cases: tuple[serialization_contract.Case, ...] = (
            serialization_contract.cases()
        )
        self.assertEqual({case.identifier for case in cases}, set(EXPECTED_CASES))
        self.assertEqual(
            [case.identifier for case in cases],
            sorted(EXPECTED_CASES),
            "case order must be deterministic",
        )
        self.assertEqual({case.category for case in cases}, {"csv", "json"})
        self.assertEqual(
            {case.surface for case in cases},
            {"page", "pdf"},
        )
        self.assertEqual(
            {str(case.fixture) for case in cases},
            {FIXTURE, BASIC_FIXTURE},
        )

    def test_committed_contract_is_complete_and_traceable(self) -> None:
        self.assertTrue(CONTRACT_PATH.is_file(), f"missing contract: {CONTRACT_PATH}")
        contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
        target = upstream.load_target()
        self.assertEqual(contract["schema_version"], 1)
        self.assertEqual(
            contract["generated_by"],
            "compat/serialization_contract.py --write-reference",
        )
        self.assertEqual(
            contract["target"],
            {
                "project": target.project,
                "version": target.version,
                "tag": target.tag,
                "commit": target.commit,
                "repository": target.repository,
            },
        )
        self.assertEqual(contract["environment"]["lockfile_sha256"], lockfile.digest())
        self.assertEqual(
            contract["environment"]["python_version"],
            upstream.load_environment().python_version,
        )
        for fixture_name in (FIXTURE, BASIC_FIXTURE):
            fixture = upstream.REPO_ROOT / fixture_name
            self.assertEqual(
                contract["resources"][fixture_name]["sha256"],
                hashlib.sha256(fixture.read_bytes()).hexdigest(),
            )

        records = contract["cases"]
        self.assertEqual([record["id"] for record in records], sorted(EXPECTED_CASES))
        for record in records:
            for key in ("callable", "category", "id", "invocation", "outcome", "surface"):
                self.assertIn(key, record)
            outcome = record["outcome"]
            self.assertIn(outcome["kind"], {"exception", "return"})
            for observed in (outcome.get("return"), outcome.get("stream")):
                if observed is None or not isinstance(observed.get("value"), str):
                    continue
                value = observed["value"]
                self.assertEqual(
                    observed["sha256"],
                    hashlib.sha256(value.encode("utf-8")).hexdigest(),
                )

        serialized = json.dumps(contract, sort_keys=True)
        self.assertNotIn(str(upstream.REPO_ROOT), serialized)

    def test_json_bytes_precision_filters_and_streams_are_exact(self) -> None:
        by_id = self._records()
        default_raw = self._text(by_id["json.pdf_default"])
        default = json.loads(default_raw)
        self.assertEqual(json.loads(self._text(by_id["json.pdf_indent_2"])), default)
        self.assertTrue(self._text(by_id["json.pdf_indent_2"]).startswith('{\n  "metadata"'))

        default_x0 = default["pages"][0]["chars"][0]["x0"]
        precision_0 = json.loads(self._text(by_id["json.pdf_precision_0"]))
        precision_3 = json.loads(self._text(by_id["json.pdf_precision_3"]))
        self.assertEqual(precision_0["pages"][0]["chars"][0]["x0"], round(default_x0, 0))
        self.assertEqual(precision_3["pages"][0]["chars"][0]["x0"], round(default_x0, 3))

        included = json.loads(self._text(by_id["json.pdf_include"]))
        included_objects = self._page_objects(included["pages"][0])
        self.assertTrue(included_objects)
        for obj in included_objects:
            self.assertIn("object_type", obj)
            self.assertLessEqual(
                set(obj), {"object_type", "page_number", "text", "x0"}
            )

        excluded = json.loads(self._text(by_id["json.pdf_exclude"]))
        for obj in self._page_objects(excluded["pages"][0]):
            self.assertNotIn("matrix", obj)
            self.assertNotIn("stream", obj)

        chars_only = json.loads(self._text(by_id["json.pdf_chars_only"]))
        self.assertIn("chars", chars_only["pages"][0])
        self.assertNotIn("rects", chars_only["pages"][0])
        self.assertNotIn("images", chars_only["pages"][0])
        no_objects = json.loads(self._text(by_id["json.pdf_no_objects"]))
        self.assertEqual(self._page_objects(no_objects["pages"][0]), [])

        self.assertEqual(self._stream_text(by_id["json.pdf_stream"]), default_raw)
        basic_text_raw = self._text(by_id["json.pdf_basic_text_default"])
        self.assertNotEqual(basic_text_raw, default_raw)
        basic_text = json.loads(basic_text_raw)
        self.assertGreater(
            len(
                {
                    obj["object_type"]
                    for obj in self._page_objects(default["pages"][0])
                }
            ),
            1,
        )
        self.assertEqual(
            {
                obj["object_type"]
                for obj in self._page_objects(basic_text["pages"][0])
            },
            {"char"},
        )
        self.assertEqual(
            self._stream_text(by_id["json.page_stream"]),
            self._text(by_id["json.page_default"]),
        )
        page_precision = json.loads(self._text(by_id["json.page_precision_3"]))
        self.assertEqual(page_precision["chars"][0]["x0"], round(default_x0, 3))
        self._assert_filter_errors(by_id, "json")

    def test_csv_bytes_precision_filters_and_streams_are_exact(self) -> None:
        by_id = self._records()
        default_raw = self._text(by_id["csv.pdf_default"])
        self.assertIn("\r\n", default_raw)
        self.assertTrue(default_raw.endswith("\r\n"))
        default_rows = list(csv.DictReader(io.StringIO(default_raw)))
        self.assertTrue(default_rows)

        precision_0 = list(
            csv.DictReader(io.StringIO(self._text(by_id["csv.pdf_precision_0"])))
        )
        precision_3 = list(
            csv.DictReader(io.StringIO(self._text(by_id["csv.pdf_precision_3"])))
        )
        first_with_x0 = next(index for index, row in enumerate(default_rows) if row["x0"])
        default_x0 = float(default_rows[first_with_x0]["x0"])
        self.assertEqual(float(precision_0[first_with_x0]["x0"]), round(default_x0, 0))
        self.assertEqual(float(precision_3[first_with_x0]["x0"]), round(default_x0, 3))

        included_reader = csv.DictReader(io.StringIO(self._text(by_id["csv.pdf_include"])))
        self.assertEqual(
            included_reader.fieldnames,
            ["object_type", "page_number", "x0", "text"],
        )
        excluded_reader = csv.DictReader(io.StringIO(self._text(by_id["csv.pdf_exclude"])))
        self.assertNotIn("matrix", excluded_reader.fieldnames)
        self.assertNotIn("stream", excluded_reader.fieldnames)

        chars_only = list(
            csv.DictReader(io.StringIO(self._text(by_id["csv.pdf_chars_only"])))
        )
        self.assertTrue(chars_only)
        self.assertEqual({row["object_type"] for row in chars_only}, {"char"})
        self.assertEqual(
            self._text(by_id["csv.pdf_no_objects"]),
            "object_type,page_number,x0,x1,y0,y1,doctop,top,bottom,width,height\r\n",
        )
        self.assertEqual(self._text(by_id["csv.page_default"]), default_raw)
        basic_text_raw = self._text(by_id["csv.pdf_basic_text_default"])
        self.assertNotEqual(basic_text_raw, default_raw)
        basic_text_rows = list(csv.DictReader(io.StringIO(basic_text_raw)))
        self.assertTrue(basic_text_rows)
        self.assertEqual({row["object_type"] for row in basic_text_rows}, {"char"})
        self.assertEqual(self._stream_text(by_id["csv.pdf_stream"]), default_raw)
        self.assertEqual(self._stream_text(by_id["csv.page_stream"]), default_raw)
        self._assert_filter_errors(by_id, "csv")

    @staticmethod
    def _page_objects(page: dict[str, object]) -> list[dict[str, object]]:
        objects: list[dict[str, object]] = []
        for key, value in page.items():
            if key.endswith("s") and isinstance(value, list):
                objects.extend(item for item in value if isinstance(item, dict))
        return objects

    @staticmethod
    def _text(record: dict[str, object]) -> str:
        outcome = record["outcome"]
        assert isinstance(outcome, dict) and outcome["kind"] == "return"
        returned = outcome["return"]
        assert isinstance(returned, dict) and returned["type"] == "builtins.str"
        value = returned["value"]
        assert isinstance(value, str)
        return value

    @staticmethod
    def _stream_text(record: dict[str, object]) -> str:
        outcome = record["outcome"]
        assert isinstance(outcome, dict) and outcome["kind"] == "return"
        returned = outcome["return"]
        assert isinstance(returned, dict)
        assert returned == {"type": "builtins.NoneType", "value": None}
        stream = outcome["stream"]
        assert isinstance(stream, dict) and stream["type"] == "builtins.str"
        value = stream["value"]
        assert isinstance(value, str)
        return value

    @staticmethod
    def _assert_filter_errors(
        by_id: dict[str, dict[str, object]], category: str
    ) -> None:
        both = by_id[f"{category}.invalid_both_filters"]["outcome"]
        assert isinstance(both, dict)
        assert both["kind"] == "exception"
        assert both["type"] == "builtins.ValueError"
        assert both["message"] == (
            "Cannot specify `include_attrs` and `exclude_attrs` at the same time."
        )
        required = by_id[f"{category}.invalid_required_exclude"]["outcome"]
        assert isinstance(required, dict)
        assert required["kind"] == "exception"
        assert required["type"] == "builtins.ValueError"
        assert required["message"] == (
            "Cannot exclude these required properties: ['object_type']"
        )

    @staticmethod
    def _records() -> dict[str, dict[str, object]]:
        contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
        return {record["id"]: record for record in contract["cases"]}


if __name__ == "__main__":
    unittest.main()
