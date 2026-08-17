"""Behavioral coverage for order-sensitive parity-report comparisons."""

import unittest

from scripts import parity_report


def object_at(text: str, x0: float) -> dict:
    return {
        "text": text,
        "x0": x0,
        "top": 10.0,
        "x1": x0 + 5.0,
        "bottom": 20.0,
    }


class OrderedSequenceComparisonTests(unittest.TestCase):
    def test_swapped_words_do_not_match(self) -> None:
        first = object_at("first", 10.0)
        second = object_at("second", 20.0)

        result = parity_report.compare_words([first, second], [second, first])

        self.assertFalse(result["order_equal"])
        self.assertEqual(result["matched"], 0)
        self.assertEqual(result["ratio"], 0.0)

    def test_extra_word_is_included_in_ordered_denominator(self) -> None:
        first = object_at("first", 10.0)
        inserted = object_at("inserted", 15.0)
        second = object_at("second", 20.0)

        result = parity_report.compare_words(
            [first, second],
            [first, inserted, second],
        )

        self.assertFalse(result["order_equal"])
        self.assertEqual(result["matched"], 1)
        self.assertEqual(result["total"], 3)
        self.assertAlmostEqual(result["ratio"], 1 / 3)


if __name__ == "__main__":
    unittest.main()
