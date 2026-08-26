"""Contracts for the maintained user-facing FAQ (ADOPT-015)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
FAQ_PATH = REPO_ROOT / "docs" / "faq.md"
README_PATH = REPO_ROOT / "README.md"
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"

EXPECTED_QUESTIONS = [
    "Does pdfplumber-rs support scanned or image-only PDFs?",
    "How does table extraction work, and will it find every table?",
    "Can it open password-protected PDFs?",
    "What happens with malformed PDFs?",
    "Which coordinate system does the project use?",
    "Is the Python package a drop-in replacement for pdfplumber?",
    "Is the WebAssembly package ready for browser production use?",
]


class FrequentlyAskedQuestionsContractTests(unittest.TestCase):
    def faq(self) -> str:
        self.assertTrue(FAQ_PATH.is_file(), "docs/faq.md is missing")
        if not FAQ_PATH.is_file():
            return ""
        return FAQ_PATH.read_text(encoding="utf-8")

    def answers(self) -> dict[str, str]:
        faq = self.faq()
        matches = list(re.finditer(r"^## (?P<question>[^\n]+)$", faq, re.MULTILINE))
        return {
            match.group("question"): faq[
                match.end() : (
                    matches[index + 1].start()
                    if index + 1 < len(matches)
                    else len(faq)
                )
            ]
            for index, match in enumerate(matches)
        }

    def test_faq_is_linked_and_has_the_exact_maintained_question_set(self) -> None:
        faq = self.faq()
        self.assertTrue(faq.startswith("# Frequently Asked Questions\n"))
        self.assertEqual(
            re.findall(r"^## ([^\n]+)$", faq, re.MULTILINE),
            EXPECTED_QUESTIONS,
        )

        readme = README_PATH.read_text(encoding="utf-8")
        self.assertIn("[Frequently Asked Questions](docs/faq.md)", readme)
        changelog = CHANGELOG_PATH.read_text(encoding="utf-8")
        self.assertIn("maintained Frequently Asked Questions page", changelog)

    def test_answers_preserve_the_public_support_boundaries(self) -> None:
        answers = self.answers()
        required_facts = {
            EXPECTED_QUESTIONS[0]: (
                "does not perform Optical Character Recognition (OCR)",
                "searchable PDF",
            ),
            EXPECTED_QUESTIONS[1]: (
                "`lattice`",
                "`stream`",
                "`explicit`",
                "not a guarantee",
            ),
            EXPECTED_QUESTIONS[2]: (
                "`Pdf::open_with_password`",
                "`password=`",
                "`--password`",
                "wrong password",
            ),
            EXPECTED_QUESTIONS[3]: (
                "`Pdf::open_with_repair`",
                "`--repair`",
                "`repair=True`",
                "Ghostscript",
                "may still fail",
            ),
            EXPECTED_QUESTIONS[4]: (
                "top-left origin",
                "`(x0, top, x1, bottom)`",
                "`doctop`",
                "`y0` and `y1`",
                "points",
            ),
            EXPECTED_QUESTIONS[5]: (
                "incomplete",
                "not a complete drop-in replacement",
                "distribution is `pdfplumber-rs`",
                "import package is `pdfplumber`",
                "fresh environment",
            ),
            EXPECTED_QUESTIONS[6]: (
                "experimental",
                "source is `0.3.0`",
                "npm release is `0.2.0`",
                "Node.js Quick Start",
                "browser end-to-end behavior is not gated",
            ),
        }

        for question, facts in required_facts.items():
            with self.subTest(question=question):
                self.assertIn(question, answers)
                for fact in facts:
                    self.assertIn(fact, answers.get(question, ""))

    def test_faq_points_to_canonical_status_and_surface_guides(self) -> None:
        faq = self.faq()
        for link in (
            "[support matrix](support.md)",
            "[readiness snapshot](readiness/v0.3.0.md)",
            "[Rust examples](../README.md#quick-start)",
            "[Python guide](../crates/pdfplumber-py/README.md)",
            "[Command-Line Interface guide](../crates/pdfplumber-cli/README.md)",
            "[WebAssembly guide](../crates/pdfplumber-wasm/README.md)",
        ):
            with self.subTest(link=link):
                self.assertIn(link, faq)

        for target in re.findall(r"\[[^\]]+\]\(([^)]+)\)", faq):
            if "://" in target:
                continue
            relative = target.split("#", 1)[0]
            with self.subTest(local_link=target):
                self.assertTrue((FAQ_PATH.parent / relative).is_file())

        for overclaim in (
            "supports every PDF",
            "finds every table",
            "is fully compatible",
            "is a complete drop-in replacement",
            "production-ready WebAssembly",
        ):
            with self.subTest(overclaim=overclaim):
                self.assertNotIn(overclaim, faq)

    def test_wasm_answer_matches_the_current_ci_boundary(self) -> None:
        matrix = tomllib.loads(MATRIX_PATH.read_text(encoding="utf-8"))
        wasm = next(surface for surface in matrix["surfaces"] if surface["id"] == "wasm")

        verified = "\n".join(wasm["ci_verified_platforms"])
        limitations = "\n".join(wasm["known_limitations"])
        self.assertIn("Node.js Quick Start", verified)
        self.assertIn("browser end-to-end behavior is not gated", limitations)
        self.assertNotIn(
            "does not run browser or Node.js end-to-end behavior",
            limitations,
        )


if __name__ == "__main__":
    unittest.main()
