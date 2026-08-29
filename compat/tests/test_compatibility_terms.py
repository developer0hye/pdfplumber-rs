"""Public terminology contracts for scoped compatibility claims (DOC-002)."""

from __future__ import annotations

import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "compatibility" / "terms.md"
README_PATH = REPO_ROOT / "README.md"
WORKFLOW_SCORECARD_PATH = REPO_ROOT / "docs" / "compatibility" / "workflows-v0.3.0.md"
UPSTREAM_PATH = REPO_ROOT / "compat" / "upstream.toml"


class CompatibilityTermsContractTests(unittest.TestCase):
    def guide(self) -> str:
        self.assertTrue(
            GUIDE_PATH.is_file(), "compatibility terminology guide is missing"
        )
        return GUIDE_PATH.read_text(encoding="utf-8")

    def test_canonical_guide_is_publicly_linked_at_the_claim_and_scorecard(
        self,
    ) -> None:
        guide = self.guide()
        readme = README_PATH.read_text(encoding="utf-8")
        workflow_scorecard = WORKFLOW_SCORECARD_PATH.read_text(encoding="utf-8")

        self.assertIn("(docs/compatibility/terms.md)", readme)
        self.assertIn("(terms.md)", workflow_scorecard)
        for target in (
            "../../compat/upstream.toml",
            "../../compat/approved_deltas.toml",
            "scorecard-v0.3.0.json",
            "workflows-v0.3.0.md",
        ):
            with self.subTest(target=target):
                self.assertIn(f"]({target})", guide)

    def test_compatible_is_bound_to_the_pinned_target_and_complete_scope(self) -> None:
        guide = self.guide()
        upstream = tomllib.loads(UPSTREAM_PATH.read_text(encoding="utf-8"))
        target = upstream["target"]

        self.assertIn(f"`pdfplumber` `{target['version']}`", guide)
        self.assertIn(f"`{target['commit']}`", guide)
        self.assertIn("## Compatible", guide)
        for field in (
            "reference",
            "surface",
            "operation",
            "options",
            "input",
            "environment",
            "artifact",
        ):
            with self.subTest(field=field):
                self.assertIn(f"`{field}`", guide)
        self.assertIn(
            "An unqualified **compatible** claim requires every observation in its "
            "named scope to be **Exact**.",
            guide,
        )
        self.assertIn(
            "A scope containing an approved deviation may be described only as "
            "**compatible with approved deviations**",
            guide,
        )
        self.assertIn(
            "Unsupported, Reference failure, Candidate failure, and Not tested are "
            "never compatible results.",
            guide,
        )

    def test_extension_is_additive_separate_and_never_parity_evidence(self) -> None:
        guide = self.guide()

        self.assertIn("## Extension", guide)
        self.assertIn("absent from the pinned Python reference", guide)
        self.assertIn("explicit namespace", guide)
        self.assertIn("never counts as parity evidence", guide)
        self.assertIn(
            "cannot hide or compensate for an incompatible reference behavior", guide
        )
        self.assertIn("Python native extension module", guide)
        self.assertIn("packaging term", guide)

    def test_approved_deviation_remains_exact_registered_and_non_exact(self) -> None:
        guide = self.guide()

        self.assertIn("## Approved deviation", guide)
        self.assertIn(
            "**Approved deviation** and the scorecard label **Approved delta** mean "
            "the same thing.",
            guide,
        )
        self.assertIn("An approval does not turn a difference into **Exact**", guide)
        self.assertIn("Wildcards are not supported", guide)
        self.assertIn("stale", guide)
        for field in (
            "fixture",
            "page",
            "api",
            "upstream_result",
            "upstream_sha256",
            "rust_result",
            "rust_sha256",
            "technical_reason",
            "compatibility_risk",
            "approving_maintainer",
            "regression_test",
            "review_condition",
        ):
            with self.subTest(field=field):
                self.assertIn(f"`{field}`", guide)
        self.assertIn("reported separately from Exact", guide)
        self.assertIn("never folded into a success percentage", guide)


if __name__ == "__main__":
    unittest.main()
