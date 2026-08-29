"""PRD master-checklist and Evidence Ledger contracts (PARITY-019)."""

from __future__ import annotations

import unittest
from pathlib import Path

from compat.harness import prd_linter


REPO_ROOT = Path(__file__).resolve().parents[2]


def document(tasks: str, evidence: str) -> str:
    return f"""# Product Requirements Document

## 8. Master Implementation Checklist

{tasks}

## 9. Known Open-Issue Mapping

- A reference to **PARITY-001** is not a task definition.

## 12. Active Work

| Task ID | Notes |
|---|---|
| `PARITY-001` | Repeated references outside Section 8 are allowed. |

## 13. Evidence Ledger

| Task ID | Date |
|---|---|
{evidence}

## 14. Decision Log
"""


class PrdLinterContractTests(unittest.TestCase):
    def test_accepts_unique_tasks_when_every_checked_task_has_evidence(self) -> None:
        text = document(
            "\n".join(
                (
                    "- [x] **PARITY-001** A completed task.",
                    "- [ ] **PYAPI-002** An incomplete task.",
                )
            ),
            "| `PARITY-001` | 2026-08-17 |",
        )

        result = prd_linter.lint_document(text)

        self.assertEqual(result.task_count, 2)
        self.assertEqual(result.checked_count, 1)
        self.assertEqual(result.evidence_count, 1)

    def test_rejects_duplicate_task_definitions_with_all_source_lines(self) -> None:
        text = document(
            "\n".join(
                (
                    "- [ ] **PYAPI-002** First definition.",
                    "- [ ] **PYAPI-002** Accidental duplicate.",
                )
            ),
            "",
        )

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"duplicate task identifier PYAPI-002.*lines 5, 6",
        ):
            prd_linter.lint_document(text)

    def test_rejects_checked_task_without_evidence_ledger_row(self) -> None:
        text = document("- [x] **PARITY-019** Completed without proof.", "")

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"checked task PARITY-019 at line 5 has no Evidence Ledger row",
        ):
            prd_linter.lint_document(text)

    def test_checked_task_requires_evidence_from_section_13_only(self) -> None:
        text = document("- [x] **PARITY-019** Completed without proof.", "")
        text = text.replace(
            "## 12. Active Work\n",
            "## 12. Active Work\n\n| `PARITY-019` | Active reference only. |\n",
        )

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"checked task PARITY-019 .* has no Evidence Ledger row",
        ):
            prd_linter.lint_document(text)

    def test_ignores_task_and_evidence_examples_inside_fenced_code(self) -> None:
        text = document(
            "\n".join(
                (
                    "- [x] **PARITY-001** The real definition.",
                    "```markdown",
                    "- [ ] **PARITY-001** An example, not a second definition.",
                    "```",
                )
            ),
            "\n".join(
                (
                    "| `PARITY-001` | 2026-08-17 |",
                    "```markdown",
                    "| `PARITY-999` | example only |",
                    "```",
                )
            ),
        )

        result = prd_linter.lint_document(text)

        self.assertEqual(result.task_count, 1)
        self.assertEqual(result.evidence_count, 1)

    def test_fenced_evidence_does_not_satisfy_a_checked_task(self) -> None:
        text = document(
            "- [x] **PARITY-019** Completed without real evidence.",
            "\n".join(
                (
                    "```markdown",
                    "| `PARITY-019` | example only |",
                    "```",
                )
            ),
        )

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"checked task PARITY-019 .* has no Evidence Ledger row",
        ):
            prd_linter.lint_document(text)

    def test_rejects_missing_or_repeated_contract_sections(self) -> None:
        valid = document("- [ ] **PARITY-019** A task.", "")

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"missing section 13",
        ):
            prd_linter.lint_document(
                valid.replace("## 13. Evidence Ledger", "## Evidence")
            )

        with self.assertRaisesRegex(
            prd_linter.PrdLintError,
            r"section 8 appears 2 times",
        ):
            prd_linter.lint_document(
                valid.replace(
                    "## 9. Known Open-Issue Mapping",
                    "## 8. Duplicate Heading\n\n## 9. Known Open-Issue Mapping",
                )
            )

    def test_repository_prd_satisfies_the_contract(self) -> None:
        result = prd_linter.lint_document(
            (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        )

        self.assertGreater(result.task_count, 700)
        self.assertEqual(result.checked_count, 73)
        self.assertGreaterEqual(result.evidence_count, result.checked_count)


if __name__ == "__main__":
    unittest.main()
