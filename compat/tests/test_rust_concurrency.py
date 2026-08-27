"""Contracts for Rust and Python concurrency guarantees (DX-008)."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE = REPO_ROOT / "docs/rust-concurrency.md"
REFERENCE = REPO_ROOT / "references/rust-concurrency.md"
README = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
CRATE_DOCS = (REPO_ROOT / "crates/pdfplumber/src/lib.rs").read_text(encoding="utf-8")
PDF_SOURCE = (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(encoding="utf-8")
PYTHON_SOURCE = (REPO_ROOT / "crates/pdfplumber-py/src/lib.rs").read_text(
    encoding="utf-8"
)
WORKFLOW = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
ROADMAP = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
CHANGELOG = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
PRD = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")


class RustConcurrencyContractTests(unittest.TestCase):
    def test_guide_defines_the_public_rust_thread_safety_matrix(self) -> None:
        self.assertTrue(GUIDE.is_file())
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        for public_type in ("`Pdf`", "`Pages<'_>`", "`Page`", "`CroppedPage`"):
            with self.subTest(public_type=public_type):
                self.assertIn(public_type, guide)
        self.assertRegex(normalized, r"(?i)Send.*Sync.*Arc<Pdf>")
        self.assertRegex(normalized, r"(?i)owned.*Page.*read-only")
        self.assertRegex(normalized, r"(?i)borrow.*Pages.*Pdf")

    def test_cache_and_resource_budget_sharing_is_explicit(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        self.assertRegex(normalized, r"(?i)caches.*immutable.*after.*open")
        self.assertRegex(
            normalized,
            r"(?i)max_total_objects.*max_total_image_bytes.*shared.*Pdf",
        )
        self.assertRegex(normalized, r"(?i)repeated.*page.*count.*again")
        self.assertIn("AtomicUsize", guide)
        self.assertRegex(PDF_SOURCE, r"total_objects:\s*AtomicUsize")
        self.assertRegex(PDF_SOURCE, r"total_image_bytes:\s*AtomicUsize")
        self.assertIn("Ordering::Relaxed", PDF_SOURCE)

    def test_rayon_contract_covers_order_pool_errors_and_platform(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        self.assertIn("`Pdf::pages_parallel`", guide)
        self.assertRegex(normalized, r"(?i)page-index order")
        self.assertRegex(normalized, r"(?i)global.*Rayon.*thread pool")
        self.assertRegex(
            normalized, r"(?i)Vec<Result<Page, PdfError>>.*does not.*cancel"
        )
        self.assertRegex(normalized, r"(?i)optional.*parallel.*WebAssembly")
        self.assertIn(
            "cargo test -p pdfplumber --features parallel --test concurrency",
            WORKFLOW,
        )

    def test_python_boundary_does_not_overpromise_parallelism(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        self.assertIn("Arc<Pdf>", PYTHON_SOURCE)
        self.assertIn("Mutex<Option<", PYTHON_SOURCE)
        self.assertNotIn("allow_threads", PYTHON_SOURCE)
        self.assertNotIn("Python::detach", PYTHON_SOURCE)
        self.assertRegex(
            normalized,
            r"(?i)(?:does not release.*Python GIL|Python GIL.*not.*release)",
        )
        self.assertRegex(normalized, r"(?i)Mutex.*cache.*not.*parallel")
        self.assertRegex(normalized, r"(?i)free-threaded.*not supported.*not verified")
        self.assertIn("- [ ] **PYAPI-023**", PRD)
        self.assertIn("PYAPI-023", guide)

    def test_primary_docs_traceability_and_roadmap_advance(self) -> None:
        for document in (README, CRATE_DOCS):
            self.assertIn("rust-concurrency", document)
        self.assertIn("thread-safety", CHANGELOG)
        self.assertTrue(REFERENCE.is_file())
        reference = REFERENCE.read_text(encoding="utf-8")
        for source in ("std::marker::Send", "std::marker::Sync", "Rayon", "PyO3"):
            with self.subTest(source=source):
                self.assertIn(source, reference)
        self.assertNotIn("### Define Rust concurrency guarantees", ROADMAP)
        self.assertIn("DX-011", ROADMAP)


if __name__ == "__main__":
    unittest.main()
