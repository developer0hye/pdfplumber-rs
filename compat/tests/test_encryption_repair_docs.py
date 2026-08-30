"""Contracts for the encryption and repair guide (DOC-013)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "encryption-and-repair.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class EncryptionRepairDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_input_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": (
                "[encryption and repair guide](docs/encryption-and-repair.md)"
            ),
            "crates/pdfplumber-cli/README.md": (
                "[encryption and repair guide](../../docs/encryption-and-repair.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[encryption and repair guide](../../docs/encryption-and-repair.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[encryption and repair guide](../../docs/encryption-and-repair.md)"
            ),
            "docs/errors-and-resource-limits.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
            "docs/faq.md": ("[encryption and repair guide](encryption-and-repair.md)"),
            "docs/pre-parity-python-migration.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
            "docs/privacy.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
            "docs/python-migration.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
            "docs/rust-api.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
            "docs/rust-errors.md": (
                "[encryption and repair guide](encryption-and-repair.md)"
            ),
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Encryption and repair:\*\* .*encryption and repair guide",
            )

    def test_pinned_python_encryption_matrix_and_permissions_are_exact(self) -> None:
        for statement in (
            "The pinned environment combines `pdfplumber==0.11.10` with `pdfminer.six==20260107`",
            "only the PDF `Standard` security handler is accepted",
            "`V=1` or `V=2` | `R=2` or `R=3` | RC4",
            "`V=4` | `R=4` | `V2` (RC4), `AESV2` (AES-128), or `Identity`",
            "`V=5` | `R=5` or `R=6` | `AESV3` (AES-256)",
            "`V=0`, `V=3`, unknown `V` values, unsupported revisions, unequal string and stream filters, and unknown crypt-filter methods fail",
            "both user and owner passwords authenticate in the generated R2-R6 matrix",
            "an empty user password opens without passing `password=`",
            "R2-R4 passwords are encoded as Latin-1",
            "R5/R6 passwords are UTF-8, limited to 127 bytes, and R6 applies SASLprep",
            "missing and incorrect non-empty passwords both become `PdfminerException` with an empty string",
            "`PDF.doc.is_printable`, `PDF.doc.is_modifiable`, and `PDF.doc.is_extractable` expose `/P` permission flags",
            "pdfplumber enumerates `PDFPage.create_pages` directly and does not enforce those flags",
            "restricted documents therefore remain extractable",
            "`EncryptMetadata=false` leaves metadata streams outside content decryption",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/pdf.py",
            "https://github.com/pdfminer/pdfminer.six/blob/20260107/pdfminer/pdfdocument.py",
            "https://github.com/pdfminer/pdfminer.six/blob/20260107/pdfminer/pdfpage.py",
            "f3d2dc69906c1c5b946916f80dce661b5f00f32f",
            "9287d0c7d64b6192139ee3645f6119784fa14d03",
            "8643a06d4a278c67f0421decfee3551ac686f7d6",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_pinned_ghostscript_repair_contract_is_complete(self) -> None:
        for statement in (
            "`PDF.open(..., repair=True)` and the public `pdfplumber.repair(...)` helper both call Ghostscript",
            "`default`, `prepress`, `printer`, `ebook`, and `screen`",
            "an explicit truthy `gs_path` wins, otherwise discovery checks `gs`, `gswin32c`, then `gswin64c`",
            "`-sstdout=%stderr -o - -sDEVICE=pdfwrite -dPDFSETTINGS=/...`",
            "a truthy password adds `-sPDFPassword=...` to the child-process argument list",
            "path input is converted to an absolute path argument",
            "a file-like input is read from its current position and sent through standard input",
            "standard output is held in a `BytesIO`; standard error is held in memory",
            "the subprocess has no timeout",
            "a nonzero status raises plain `Exception` with decoded standard error",
            "when `outfile` is supplied, `pdfplumber.repair` writes the bytes and returns `None`",
            "without `outfile`, it returns a caller-owned `BytesIO` positioned at zero",
            "`PDF.open(repair=True)` owns and closes the repaired stream but not an original caller-owned stream",
            "repair runs before the normal parser and does not make every malformed input recoverable",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/repair.py",
            "2e4df9aaf0c034077e4ef68b5c776c975fa1eed4",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_rust_encryption_and_native_repair_boundaries_are_exact(self) -> None:
        for statement in (
            "`open_path_with_password`, `open_bytes_with_password`, and `open_reader_with_password`",
            "the password type is `&[u8]`",
            "passwordless opening auto-decrypts a document whose user password is empty",
            "after that automatic empty-user decryption, any supplied password is ignored",
            "a supplied password is ignored for an unencrypted document",
            "missing credentials return `PdfErrorKind::PasswordRequired`; incorrect credentials return `PdfErrorKind::InvalidPassword`",
            "unsupported encryption structure remains `PdfErrorKind::Parse`",
            "user-password extraction succeeded for R2, R3, R4 with RC4, R4 with AES-128, and R5/R6 with AES-256",
            "owner-password extraction succeeded for R5/R6 but produced empty text for the generated R2-R4 probes",
            "do not rely on legacy owner-password extraction until that residual is fixed",
            "the current facade neither exposes nor enforces document permission flags",
            "`Pdf::open_bytes_with_repair` is byte-only and accepts `RepairOptions`",
            "`rebuild_xref`, `fix_stream_lengths`, and `remove_broken_objects` all default to `true`",
            "loading with lopdf must succeed before native repair can run",
            "saving always writes a fresh cross-reference table",
            "direct missing or incorrect stream lengths are rewritten; indirect `/Length` references are skipped",
            "dangling references in arrays, dictionaries, and stream dictionaries are recursively replaced with `Null`",
            "`RepairResult::has_repairs()` reports whether the log is non-empty, not whether serialization rewrote bytes",
            "native repair has no password argument and is not an encrypted-document repair contract",
            "the original and repaired byte sequences are both checked against `max_input_bytes`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        rust_source = (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(
            encoding="utf-8"
        )
        for method in (
            "open_path_with_password",
            "open_bytes_with_password",
            "open_reader_with_password",
            "open_bytes_with_repair",
        ):
            with self.subTest(rust_method=method):
                self.assertIn(f"pub fn {method}", rust_source)

        repair_source = (REPO_ROOT / "crates/pdfplumber-core/src/repair.rs").read_text(
            encoding="utf-8"
        )
        for field in (
            "pub rebuild_xref: bool",
            "pub fix_stream_lengths: bool",
            "pub remove_broken_objects: bool",
            "!self.log.is_empty()",
        ):
            with self.subTest(repair_contract=field):
                self.assertIn(field, repair_source)

        backend_source = (
            REPO_ROOT / "crates/pdfplumber-parse/src/lopdf_backend.rs"
        ).read_text(encoding="utf-8")
        for behavior in (
            "lopdf::Document::load_mem(bytes)",
            "repair_stream_lengths(&mut doc, &mut result)",
            "repair_broken_references(&mut doc, &mut result)",
            "doc.save_to(&mut buf)",
            "Ok(lopdf::Object::Reference(_))",
            "lopdf::Object::Null",
        ):
            with self.subTest(repair_backend=behavior):
                self.assertIn(behavior, backend_source)

    def test_adapter_security_and_claim_boundaries_remain_explicit(self) -> None:
        for statement in (
            "The Python adapter accepts `password=` on paths and seekable binary streams",
            "maps Rust password failures to the compatibility `PdfminerException`",
            "Python `repair=True` uses the bundled Ghostscript helper, not Rust native repair",
            "the current package does not export the upstream public `pdfplumber.repair(...)` helper",
            "all extraction commands accept `--password` and `--repair`; `validate` accepts only `--password`",
            "when both `--password` and `--repair` are present, the CLI performs password-only opening and skips repair",
            "the CLI password is a command-line argument and may be visible to shell history and local process inspection",
            "the WebAssembly adapter exposes neither password-aware opening nor repair",
            "a password is not a sandbox",
            "repair is a lossy rewrite boundary",
            "run untrusted parsing and Ghostscript with host-enforced CPU, memory, file, and time limits",
            "do not log passwords, decrypted bytes, original paths, Ghostscript arguments, or protected source chains",
            "encryption and repair documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "DOC-013 changes no runtime behavior",
            "../crates/pdfplumber/src/pdf.rs",
            "../crates/pdfplumber-core/src/repair.rs",
            "../crates/pdfplumber-parse/src/lopdf_backend.rs",
            "../crates/pdfplumber-py/src/lib.rs",
            "../crates/pdfplumber-py/python/pdfplumber/repair.py",
            "../crates/pdfplumber-cli/src/shared.rs",
            "../crates/pdfplumber-cli/src/cli.rs",
            "../crates/pdfplumber-wasm/src/lib.rs",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## Pinned Python encryption.*"
                r"## Pinned Python Ghostscript repair.*"
                r"## Current Rust library.*"
                r"## Current adapter matrix.*"
                r"## Security and operations.*"
                r"## Validation and provenance.*"
                r"## Claim boundary",
                self.guide,
            )
        )
        self.assertGreaterEqual(self.guide.count("```python"), 2)
        self.assertGreaterEqual(self.guide.count("```rust"), 2)
        self.assertGreaterEqual(self.guide.count("```console"), 2)
        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))

        python_init = (
            REPO_ROOT / "crates/pdfplumber-py/python/pdfplumber/__init__.py"
        ).read_text(encoding="utf-8")
        python_repair = (
            REPO_ROOT / "crates/pdfplumber-py/python/pdfplumber/repair.py"
        ).read_text(encoding="utf-8")
        self.assertEqual(python_init.count('__all__ = ["_native"]'), 1)
        self.assertIn("def _repair(", python_repair)
        self.assertNotIn("def repair(", python_repair)

        cli_source = (REPO_ROOT / "crates/pdfplumber-cli/src/shared.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "If password is provided with repair, fall back to password-only open",
            cli_source,
        )
        self.assertIn("Pdf::open_bytes_with_password", cli_source)
        self.assertIn("Pdf::open_with_repair", cli_source)

        wasm_source = (REPO_ROOT / "crates/pdfplumber-wasm/src/lib.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("Pdf::open_bytes(data, None)", wasm_source)
        self.assertNotIn("password", wasm_source)
        self.assertNotIn("repair", wasm_source)


if __name__ == "__main__":
    unittest.main()
