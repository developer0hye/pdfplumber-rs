"""Contracts for the public Rust deprecation lifecycle (DX-015)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "docs/rust-deprecation-policy.md"
README = (ROOT / "README.md").read_text(encoding="utf-8")
RUST_API = (ROOT / "docs/rust-api.md").read_text(encoding="utf-8")
CRATE_DOCS = (ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
CHANGELOG = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
ROADMAP = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
REFERENCE_INDEX = (ROOT / "references/INDEX.md").read_text(encoding="utf-8")
SUPPORT_SOURCE = (ROOT / "support-matrix.toml").read_text(encoding="utf-8")
DEPRECATED_ATTRIBUTE = re.compile(
    r"(?m)^\s*#\[deprecated(?:\((?P<body>[^\]]+)\))?\]"
)


class RustDeprecationPolicyContractTests(unittest.TestCase):
    def policy(self) -> str:
        self.assertTrue(POLICY_PATH.is_file())
        if not POLICY_PATH.is_file():
            return ""
        return POLICY_PATH.read_text(encoding="utf-8")

    def test_policy_scope_is_the_stable_rust_facade(self) -> None:
        policy = self.policy()
        normalized = " ".join(policy.split())

        self.assertRegex(normalized, r"(?i)stable.*pdfplumber.*facade")
        self.assertRegex(normalized, r"(?i)doc\(hidden\).*not.*deprecat")
        for separate_surface in (
            "pdfplumber-core",
            "pdfplumber-parse",
            "Python",
            "Command-Line Interface",
            "serde-json-v1",
        ):
            with self.subTest(surface=separate_surface):
                self.assertIn(separate_surface, policy)

        for primary_document in (README, RUST_API, CRATE_DOCS):
            self.assertIn("rust-deprecation-policy.md", primary_document)

    def test_window_counts_two_subsequent_published_minor_releases(self) -> None:
        normalized = " ".join(self.policy().split())

        self.assertRegex(normalized, r"(?i)two subsequent.*minor releases")
        self.assertRegex(normalized, r"(?i)patch releases.*do not count")
        self.assertRegex(normalized, r"(?i)skipped.*minor.*do not count")
        self.assertRegex(normalized, r"0\.3.*0\.4.*0\.5.*0\.6\.0")
        self.assertRegex(normalized, r"1\.2.*1\.3.*1\.4.*2\.0\.0")
        self.assertRegex(normalized, r"(?i)after 1\.0.*major")

    def test_declarations_require_since_and_actionable_replacement_notes(self) -> None:
        policy = self.policy()
        normalized = " ".join(policy.split())

        self.assertIn('#[deprecated(since = "X.Y.Z", note = "Use replacement")]', policy)
        self.assertRegex(normalized, r"(?i)replacement.*available.*before.*deprecat")
        self.assertRegex(normalized, r"(?i)rustdoc.*since.*note")

        source_paths = sorted((ROOT / "crates/pdfplumber/src").rglob("*.rs"))
        self.assertTrue(source_paths)
        for source_path in source_paths:
            source = source_path.read_text(encoding="utf-8")
            for match in DEPRECATED_ATTRIBUTE.finditer(source):
                body = match.group("body")
                with self.subTest(path=source_path.relative_to(ROOT), body=body):
                    self.assertIsNotNone(body, "bare #[deprecated] is forbidden")
                    if body is None:
                        continue
                    self.assertRegex(body, r'since\s*=\s*"\d+\.\d+\.\d+"')
                    note_match = re.search(r'note\s*=\s*"(?P<note>[^"]+)"', body)
                    self.assertIsNotNone(note_match)
                    if note_match is not None:
                        self.assertRegex(
                            note_match.group("note"),
                            r"(?i)\b(use|replace|migrate|renamed)\b",
                        )

    def test_release_communication_and_removal_gates_are_explicit(self) -> None:
        normalized = " ".join(self.policy().split())

        self.assertIn("### Deprecated", self.policy())
        self.assertIn("**Migration:** Breaking:", self.policy())
        self.assertRegex(normalized, r"(?i)cargo-semver-checks.*before.*remov")
        self.assertRegex(normalized, r"(?i)rustdoc.*changelog.*release notes")
        self.assertRegex(normalized, r"(?i)removal.*SemVer-incompatible release")

    def test_only_safety_or_unsoundness_can_shorten_the_window(self) -> None:
        normalized = " ".join(self.policy().split())

        self.assertRegex(normalized, r"(?i)only.*safety.*unsoundness")
        self.assertRegex(normalized, r"(?i)maintenance.*convenience.*not.*exception")
        self.assertRegex(normalized, r"(?i)smallest.*break")
        self.assertRegex(normalized, r"(?i)public.*rationale.*migration")

    def test_changelog_sources_and_roadmap_are_traceable(self) -> None:
        self.assertRegex(CHANGELOG, r"(?is)deprecation policy.*two.*minor")

        reference_path = ROOT / "references/rust-deprecation.md"
        self.assertTrue(reference_path.is_file())
        if reference_path.is_file():
            reference = reference_path.read_text(encoding="utf-8")
            self.assertIn("Rust Reference", reference)
            self.assertIn("Cargo SemVer Compatibility", reference)
        self.assertIn("rust-deprecation.md", REFERENCE_INDEX)
        self.assertIn("docs/rust-deprecation-policy.md", SUPPORT_SOURCE)
        self.assertIn("compat/tests/test_rust_deprecation_policy.py", SUPPORT_SOURCE)

        self.assertNotIn("### Define a deprecation policy", ROADMAP)
        self.assertIn("DX-016", ROADMAP)


if __name__ == "__main__":
    unittest.main()
