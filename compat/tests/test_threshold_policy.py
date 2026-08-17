from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from compat.harness import threshold_policy


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "threshold-policy.yml"
POLICY_SCRIPT = REPO_ROOT / "scripts" / "check_threshold_policy.py"


class ThresholdExtractionTests(unittest.TestCase):
    def test_extracts_named_and_case_specific_percentage_thresholds(self) -> None:
        source = """
const CHAR_THRESHOLD: f64 = 0.95;

#[test]
fn direct_assertion() {
    assert!(cf1.f1 >= 0.90);
}

cross_validate!(cv_literal, "literal.pdf", 0.80, CHAR_THRESHOLD);
"""

        extracted = threshold_policy.extract_thresholds("tests/parity.rs", source)

        self.assertEqual(extracted["tests/parity.rs::const::CHAR_THRESHOLD"], 0.95)
        self.assertEqual(
            extracted["tests/parity.rs::fn::direct_assertion::cf1.f1::1"], 0.90
        )
        self.assertEqual(
            extracted["tests/parity.rs::cross_validate::cv_literal::char"], 0.80
        )
        self.assertEqual(
            extracted["tests/parity.rs::cross_validate::cv_literal::word"], 0.95
        )

    def test_finds_constant_literal_and_removed_threshold_regressions(self) -> None:
        before = {
            "a.rs::const::CHAR_THRESHOLD": 0.95,
            "a.rs::cross_validate::fixture::word": 0.80,
            "b.rs::fn::table::accuracy::1": 0.90,
            "b.rs::fn::newly_stricter::f1::1": 0.50,
        }
        after = {
            "a.rs::const::CHAR_THRESHOLD": 0.90,
            "a.rs::cross_validate::fixture::word": 0.20,
            "b.rs::fn::newly_stricter::f1::1": 0.75,
        }

        reductions = threshold_policy.find_reductions(before, after)

        self.assertEqual(
            [(item.key, item.before, item.after) for item in reductions],
            [
                ("a.rs::const::CHAR_THRESHOLD", 0.95, 0.90),
                ("a.rs::cross_validate::fixture::word", 0.80, 0.20),
                ("b.rs::fn::table::accuracy::1", 0.90, None),
            ],
        )

    def test_ignores_comparison_text_in_comments_and_rust_strings(self) -> None:
        source = (
            "const REAL_THRESHOLD: f64 = 0.95;\n"
            "// const COMMENT_THRESHOLD: f64 = 0.10;\n"
            'const MESSAGE: &str = r#"const RAW_THRESHOLD: f64 = 0.20;"#;\n'
            "fn lifetime<'a>(value: &'a Score) {\n"
            '    let message = "fake.f1 >= 0.10";\n'
            "    assert!(value.f1 >= REAL_THRESHOLD);\n"
            "}\n"
        )

        extracted = threshold_policy.extract_thresholds("tests/strings.rs", source)

        self.assertEqual(
            extracted,
            {
                "tests/strings.rs::const::REAL_THRESHOLD": 0.95,
                "tests/strings.rs::fn::lifetime::value.f1::1": 0.95,
            },
        )

    def test_shared_constant_reduction_has_one_evidence_identity(self) -> None:
        before = threshold_policy.extract_thresholds(
            "tests/shared.rs",
            """
const CHAR_THRESHOLD: f64 = 0.95;
cross_validate!(first, "first.pdf", CHAR_THRESHOLD, CHAR_THRESHOLD);
cross_validate!(second, "second.pdf", CHAR_THRESHOLD, CHAR_THRESHOLD);
""",
        )
        after = threshold_policy.extract_thresholds(
            "tests/shared.rs",
            """
const CHAR_THRESHOLD: f64 = 0.90;
cross_validate!(first, "first.pdf", CHAR_THRESHOLD, CHAR_THRESHOLD);
cross_validate!(second, "second.pdf", CHAR_THRESHOLD, CHAR_THRESHOLD);
""",
        )

        reductions = threshold_policy.find_reductions(before, after)

        self.assertEqual(
            reductions,
            (
                threshold_policy.Reduction(
                    "tests/shared.rs::const::CHAR_THRESHOLD", 0.95, 0.90
                ),
            ),
        )

    def test_extracts_strict_rust_and_python_percentage_comparisons(self) -> None:
        rust = threshold_policy.extract_thresholds(
            "tests/strict.rs",
            "fn strict() { assert!(match_rate > 0.80); }\n",
        )
        python = threshold_policy.extract_thresholds(
            "compat/strict.py",
            "RATE_THRESHOLD = 0.95\n"
            "def strict(score):\n"
            "    assert score >= RATE_THRESHOLD\n",
        )

        self.assertEqual(
            rust,
            {"tests/strict.rs::fn::strict::match_rate::1": 0.80},
        )
        self.assertEqual(
            python,
            {
                "compat/strict.py::const::RATE_THRESHOLD": 0.95,
                "compat/strict.py::python::score::1": 0.95,
            },
        )


class ThresholdApprovalTests(unittest.TestCase):
    HEAD_SHA = "a" * 40
    REDUCTION = threshold_policy.Reduction(
        key="tests/parity.rs::const::CHAR_THRESHOLD",
        before=0.95,
        after=0.90,
    )

    def evidence(self) -> str:
        marker = threshold_policy.format_evidence_marker(self.REDUCTION)
        return f"""
## 13. Evidence Ledger

| Task ID | Date | Agent | Commit / PR | Test evidence | Notes |
|---|---|---|---|---|---|
| `PARITY-999` | 2026-08-17 | Maintainer | PR #1 | `{marker}` | rationale |

## 14. Decision Log
"""

    def approved_review(self, *, commit_id: str | None = None) -> list[dict[str, str]]:
        return [
            {
                "login": "maintainer",
                "state": "APPROVED",
                "commit_id": commit_id or self.HEAD_SHA,
            }
        ]

    def test_reduction_requires_both_ledger_evidence_and_current_maintainer_review(
        self,
    ) -> None:
        with self.assertRaisesRegex(
            threshold_policy.ThresholdPolicyError, "Evidence Ledger"
        ):
            threshold_policy.enforce_policy(
                [self.REDUCTION],
                prd_text="## 13. Evidence Ledger\n\n## 14. Decision Log\n",
                reviews=self.approved_review(),
                permissions={"maintainer": "maintain"},
                head_sha=self.HEAD_SHA,
            )

        with self.assertRaisesRegex(
            threshold_policy.ThresholdPolicyError, "maintainer approval"
        ):
            threshold_policy.enforce_policy(
                [self.REDUCTION],
                prd_text=self.evidence(),
                reviews=[],
                permissions={},
                head_sha=self.HEAD_SHA,
            )

        result = threshold_policy.enforce_policy(
            [self.REDUCTION],
            prd_text=self.evidence(),
            reviews=self.approved_review(),
            permissions={"maintainer": "maintain"},
            head_sha=self.HEAD_SHA,
        )
        self.assertEqual(result.approver, "maintainer")

    def test_rejects_stale_approval_and_non_maintainer_permission(self) -> None:
        with self.assertRaisesRegex(
            threshold_policy.ThresholdPolicyError, "maintainer approval"
        ):
            threshold_policy.enforce_policy(
                [self.REDUCTION],
                prd_text=self.evidence(),
                reviews=self.approved_review(commit_id="b" * 40),
                permissions={"maintainer": "admin"},
                head_sha=self.HEAD_SHA,
            )

        with self.assertRaisesRegex(
            threshold_policy.ThresholdPolicyError, "maintainer approval"
        ):
            threshold_policy.enforce_policy(
                [self.REDUCTION],
                prd_text=self.evidence(),
                reviews=self.approved_review(),
                permissions={"maintainer": "write"},
                head_sha=self.HEAD_SHA,
            )

    def test_no_reduction_needs_no_exception_evidence(self) -> None:
        result = threshold_policy.enforce_policy(
            [],
            prd_text="",
            reviews=[],
            permissions={},
            head_sha=self.HEAD_SHA,
        )
        self.assertIsNone(result.approver)


class ThresholdWorkflowTests(unittest.TestCase):
    def test_workflow_executes_trusted_policy_on_pr_and_review_events(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("pull_request_target:", source)
        self.assertIn("pull_request_review:", source)
        self.assertIn("pull-requests: read", source)
        self.assertIn("ref: ${{ github.event.pull_request.base.sha }}", source)
        self.assertIn("refs/pull/${PR_NUMBER}/head", source)
        self.assertIn("python scripts/check_threshold_policy.py", source)
        self.assertNotIn("checkout@v4\n        with:\n          ref: ${{ github.event.pull_request.head.sha }}", source)


class ThresholdCliTests(unittest.TestCase):
    def git(self, repo: Path, *arguments: str) -> str:
        completed = subprocess.run(
            ["git", "-C", str(repo), *arguments],
            check=True,
            capture_output=True,
            text=True,
        )
        return completed.stdout.strip()

    def test_cli_reads_git_revisions_and_fails_closed_on_a_real_reduction(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            source = repo / "crates" / "pdfplumber" / "tests" / "parity.rs"
            source.parent.mkdir(parents=True)
            (repo / "PRD.md").write_text(
                "## 13. Evidence Ledger\n\n## 14. Decision Log\n",
                encoding="utf-8",
            )
            source.write_text(
                "const CHAR_THRESHOLD: f64 = 0.95;\n", encoding="utf-8"
            )
            self.git(repo, "init", "-q")
            self.git(repo, "config", "user.name", "Policy Test")
            self.git(repo, "config", "user.email", "policy@example.com")
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-q", "-m", "base")
            base = self.git(repo, "rev-parse", "HEAD")

            source.write_text(
                "const CHAR_THRESHOLD: f64 = 0.90;\n", encoding="utf-8"
            )
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-q", "-m", "reduce")
            head = self.git(repo, "rev-parse", "HEAD")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(POLICY_SCRIPT),
                    "--repo-root",
                    str(repo),
                    "--base",
                    base,
                    "--head",
                    head,
                    "--head-sha",
                    head,
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 1)
            self.assertIn(
                "reductions require --repository and --pull-request",
                completed.stderr,
            )


if __name__ == "__main__":
    unittest.main()
