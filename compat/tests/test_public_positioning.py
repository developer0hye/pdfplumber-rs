"""Contracts for concise search and registry positioning (ADOPT-014)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
SUPPORT_PATH = REPO_ROOT / "docs" / "support.md"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_metadata.py"

EXPECTED_POSITIONING = (
    "Evidence-driven PDF extraction for Rust, with an alpha Python pdfplumber "
    "migration path."
)
DESCRIPTION_MARKER = "evidence-driven pdf extraction"
FORBIDDEN_OVERCLAIMS = (
    "100% compatible",
    "complete drop-in",
    "complete replacement",
    "fully compatible",
    "full drop-in",
)


def load_toml(relative: str) -> dict:
    return tomllib.loads((REPO_ROOT / relative).read_text(encoding="utf-8"))


class PublicPositioningContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = load_toml("support-matrix.toml")
        self.surfaces = {
            surface["id"]: surface for surface in self.matrix["surfaces"]
        }

    def test_search_description_has_one_concise_alpha_position(self) -> None:
        positioning = self.matrix["positioning"]

        self.assertEqual(positioning, EXPECTED_POSITIONING)
        self.assertEqual(self.matrix["github_description"], positioning)
        self.assertLessEqual(len(positioning), 160)
        self.assertNotRegex(positioning, r"[`\n]")
        lowered = positioning.lower()
        for phrase in FORBIDDEN_OVERCLAIMS:
            with self.subTest(phrase=phrase):
                self.assertNotIn(phrase, lowered)

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        opening = re.search(r"^\*\*(?P<text>.+)\*\*$", readme, re.MULTILINE)
        self.assertIsNotNone(opening, "README has no bold positioning line")
        if opening is not None:
            self.assertEqual(opening.group("text").replace("`", ""), positioning)

    def test_registry_descriptions_share_positioning_and_maturity(self) -> None:
        for surface in self.matrix["surfaces"]:
            configured = surface["registry_description"]
            observed = surface["observed_registry_description"]
            with self.subTest(surface=surface["id"], field="configured"):
                self.assertIn(DESCRIPTION_MARKER, configured.lower())
                self.assertIn(surface["maturity"], configured.lower())
                self.assertLessEqual(len(configured), 200)
                self.assertNotRegex(configured, r"[`\n]")
                for phrase in FORBIDDEN_OVERCLAIMS:
                    self.assertNotIn(phrase, configured.lower())
            with self.subTest(surface=surface["id"], field="observed"):
                self.assertTrue(observed.strip())

            manifest = load_toml(surface["manifest"])
            self.assertEqual(
                manifest["package"]["description"],
                configured,
                f"{surface['manifest']} description drifted",
            )

        python_project = load_toml("crates/pdfplumber-py/pyproject.toml")["project"]
        self.assertEqual(
            python_project["description"],
            self.surfaces["python"]["registry_description"],
        )

    def test_all_publishable_source_descriptions_share_the_claim_boundary(self) -> None:
        manifests = (
            "crates/pdfplumber/Cargo.toml",
            "crates/pdfplumber-core/Cargo.toml",
            "crates/pdfplumber-parse/Cargo.toml",
            "crates/pdfplumber-cli/Cargo.toml",
            "crates/pdfplumber-py/Cargo.toml",
            "crates/pdfplumber-wasm/Cargo.toml",
        )
        for relative in manifests:
            package = load_toml(relative)["package"]
            description = package["description"]
            maturity = package["metadata"]["pdfplumber-rs"]["maturity"]
            with self.subTest(manifest=relative):
                self.assertIn(DESCRIPTION_MARKER, description.lower())
                self.assertIn(maturity, description.lower())
                for phrase in FORBIDDEN_OVERCLAIMS:
                    self.assertNotIn(phrase, description.lower())

    def test_checker_and_support_page_expose_description_drift(self) -> None:
        checker = CHECKER_PATH.read_text(encoding="utf-8")
        self.assertIn("registry_description", checker)
        self.assertIn("github_description", checker)

        support = SUPPORT_PATH.read_text(encoding="utf-8")
        self.assertIn("## Positioning and registry descriptions", support)
        self.assertIn(EXPECTED_POSITIONING, support)
        self.assertIn("Configured description", support)
        self.assertIn("Observed published description", support)
        self.assertIn("Awaiting next publication", support)


if __name__ == "__main__":
    unittest.main()
