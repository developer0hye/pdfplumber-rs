from __future__ import annotations

import inspect
import subprocess
import unittest

from scripts import check_doc_quickstarts as quick_starts


class RustQuickStartContractTests(unittest.TestCase):
    def primary_snippet(self) -> str:
        _, snippets = quick_starts.surface_snippets("rust")
        self.assertTrue(snippets, "README Rust Quick Start has no example")
        return snippets[0]

    def test_primary_example_is_short_copy_pasteable_and_fallible(self) -> None:
        snippet = self.primary_snippet()

        self.assertLessEqual(len(snippet.splitlines()), 15)
        self.assertIn('Pdf::open_path("document.pdf", None)?', snippet)
        self.assertIn("let page = page?;", snippet)
        self.assertIn("extract_text(&TextOptions::default())", snippet)
        self.assertRegex(snippet, r"print(?:ln)?!")
        self.assertRegex(
            snippet,
            r"fn main\(\) -> Result<\(\), Box<dyn std::error::Error>>",
        )
        self.assertGreaterEqual(snippet.count("?"), 2)
        self.assertIn("Ok(())", snippet)
        self.assertNotRegex(snippet, r"\.(?:unwrap|expect)\(")
        self.assertNotIn(quick_starts.PRIMARY_RUST_OUTPUT_MARKER, snippet)

    def test_static_checker_owns_the_primary_example_contract(self) -> None:
        quick_starts.validate_primary_rust_quick_start(self.primary_snippet())
        source = inspect.getsource(quick_starts.check_static_contract)
        self.assertIn("validate_primary_rust_quick_start", source)

    def test_runtime_checker_requires_useful_extracted_text(self) -> None:
        marker = "The quick brown fox jumps over the lazy dog."
        quick_starts.require_useful_rust_output(marker)
        with self.assertRaisesRegex(
            quick_starts.QuickStartError,
            "did not print extracted fixture text",
        ):
            quick_starts.require_useful_rust_output("")

        source = inspect.getsource(quick_starts.run_rust_quick_starts)
        self.assertIn("require_useful_rust_output", source)

    def test_runtime_checker_requires_a_reported_non_panic_error(self) -> None:
        reported = subprocess.CompletedProcess(
            args=["quickstart_1"],
            returncode=1,
            stdout="",
            stderr="Error: file not found\n",
        )
        quick_starts.require_reported_rust_error(reported)

        for completed in (
            subprocess.CompletedProcess(["quickstart_1"], 0, "", ""),
            subprocess.CompletedProcess(["quickstart_1"], 1, "", ""),
            subprocess.CompletedProcess(
                ["quickstart_1"], 101, "", "thread panicked at source.rs:1"
            ),
        ):
            with (
                self.subTest(
                    returncode=completed.returncode, stderr=completed.stderr
                ),
                self.assertRaises(quick_starts.QuickStartError),
            ):
                quick_starts.require_reported_rust_error(completed)

        source = inspect.getsource(quick_starts.run_rust_quick_starts)
        self.assertIn("require_reported_rust_error", source)


if __name__ == "__main__":
    unittest.main()
