"""Contracts for the concise user-facing product roadmap."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ROADMAP_PATH = REPO_ROOT / "ROADMAP.md"
PRD_PATH = REPO_ROOT / "PRD.md"
TASK_LINK_PATTERN = re.compile(
    r"\[`(?P<identifier>[A-Z]+(?:-[A-Z]+)*-[0-9]{3})`\]"
    r"\(PRD\.md#[^)]+\)"
)
TASK_PATTERN = re.compile(
    r"^- \[([ xX])\] \*\*([A-Z]+(?:-[A-Z]+)*-[0-9]{3})\*\*",
    re.MULTILINE,
)


class PublicRoadmapContractTests(unittest.TestCase):
    def test_roadmap_is_concise_ordered_and_links_open_tasks(self) -> None:
        self.assertTrue(ROADMAP_PATH.is_file())
        if not ROADMAP_PATH.is_file():
            return

        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        self.assertEqual(
            re.findall(r"^## (Now|Next|Later)$", roadmap, re.MULTILINE),
            ["Now", "Next", "Later"],
        )
        self.assertIn("priorities, not release-date promises", roadmap)
        self.assertIn("(docs/readiness/v0.3.0.md)", roadmap)
        self.assertIn("(docs/support.md)", roadmap)
        self.assertNotRegex(roadmap, TASK_PATTERN)

        task_ids = TASK_LINK_PATTERN.findall(roadmap)
        self.assertGreaterEqual(len(task_ids), 12)
        self.assertLessEqual(len(task_ids), 24)
        self.assertEqual(len(task_ids), len(set(task_ids)))

        prd = PRD_PATH.read_text(encoding="utf-8")
        task_states: dict[str, list[bool]] = {}
        for marker, identifier in TASK_PATTERN.findall(prd):
            task_states.setdefault(identifier, []).append(marker.lower() == "x")
        for identifier in task_ids:
            with self.subTest(task=identifier):
                self.assertEqual(len(task_states.get(identifier, [])), 1)
                self.assertFalse(task_states[identifier][0])

        sections = re.split(r"^## (?:Now|Next|Later)$", roadmap, flags=re.MULTILINE)[1:]
        self.assertEqual(len(sections), 3)
        for horizon, section in zip(("Now", "Next", "Later"), sections, strict=True):
            with self.subTest(horizon=horizon):
                self.assertGreaterEqual(
                    len(re.findall(r"^### ", section, re.MULTILINE)), 2
                )
                self.assertTrue(TASK_LINK_PATTERN.search(section))

    def test_readme_links_public_roadmap_and_detailed_evidence_separately(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("[public roadmap](ROADMAP.md)", readme)
        self.assertIn("[detailed evidence ledger](PRD.md#13-evidence-ledger)", readme)

    def test_comparison_guide_links_to_the_public_roadmap(self) -> None:
        comparison = (REPO_ROOT / "docs" / "comparison.md").read_text(encoding="utf-8")

        self.assertIn("[public roadmap](../ROADMAP.md)", comparison)


if __name__ == "__main__":
    unittest.main()
