"""Behavioral coverage for exact dictionary-structure comparison."""

import unittest

from scripts import parity_report


class DictionaryStructureComparisonTests(unittest.TestCase):
    def test_key_spelling_must_match_exactly(self) -> None:
        expected = [{"text": "A", "page_number": 1}]
        actual = [{"text": "A", "pageNumber": 1}]

        result = parity_report.compare_dictionary_sequence(expected, actual)

        self.assertFalse(result["structure_equal"])
        self.assertEqual(result["matched"], 0)
        self.assertEqual(result["total"], 1)

    def test_scalar_types_do_not_coerce(self) -> None:
        expected = [{"count": 1, "enabled": True}]
        actual = [{"count": 1.0, "enabled": 1}]

        result = parity_report.compare_dictionary_sequence(expected, actual)

        self.assertFalse(result["structure_equal"])
        self.assertEqual(result["matched"], 0)

    def test_nested_none_placement_is_positional(self) -> None:
        expected = [{"metadata": {"colors": [None, "red"]}}]
        actual = [{"metadata": {"colors": ["red", None]}}]

        result = parity_report.compare_dictionary_sequence(expected, actual)

        self.assertFalse(result["structure_equal"])
        self.assertEqual(result["matched"], 0)

    def test_tuple_and_list_are_distinct_nested_types(self) -> None:
        expected = [{"matrix": (1.0, 0.0, 0.0, 1.0, 2.0, 3.0)}]
        actual = [{"matrix": [1.0, 0.0, 0.0, 1.0, 2.0, 3.0]}]

        result = parity_report.compare_dictionary_sequence(expected, actual)

        self.assertFalse(result["structure_equal"])
        self.assertEqual(result["matched"], 0)

    def test_table_none_is_not_treated_as_an_empty_string(self) -> None:
        result = parity_report.compare_tables([[[None]]], [[[""]]])

        self.assertFalse(result["structure_equal"])
        self.assertEqual(result["cell_ratio"], 0.0)

    def test_key_order_and_scalar_values_do_not_change_structure(self) -> None:
        expected = [{"text": "first", "box": {"x": 1.0, "y": 2.0}}]
        actual = [{"box": {"y": 200.0, "x": 100.0}, "text": "second"}]

        result = parity_report.compare_dictionary_sequence(expected, actual)

        self.assertTrue(result["structure_equal"])
        self.assertEqual(result["matched"], 1)
        self.assertEqual(result["ratio"], 1.0)

    def test_python_page_preserves_full_upstream_dictionaries(self) -> None:
        char = {
            "text": "A",
            "x0": 1.0,
            "top": 2.0,
            "x1": 3.0,
            "bottom": 4.0,
            "object_type": "char",
            "page_number": 1,
            "mcid": None,
            "matrix": (1.0, 0.0, 0.0, 1.0, 1.0, 2.0),
        }
        word = {
            "text": "A",
            "x0": 1.0,
            "top": 2.0,
            "x1": 3.0,
            "bottom": 4.0,
            "direction": "ltr",
            "chars": [char],
        }

        class Page:
            chars = [char]

            @staticmethod
            def extract_words() -> list[dict]:
                return [word]

            @staticmethod
            def extract_text() -> str:
                return "A"

            @staticmethod
            def extract_tables() -> list:
                return []

        result = parity_report.python_page(Page(), 1)

        self.assertEqual(result["chars"], [char])
        self.assertEqual(result["words"], [word])


if __name__ == "__main__":
    unittest.main()
