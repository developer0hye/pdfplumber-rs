# pdfplumber-py — Agent Working Memory

```bash
# Requires Python + maturin
maturin develop
python -c "import pdfplumber; print('ok')"
cargo check -p pdfplumber-py
```

**~40 tests | ~1,500 lines | 1 file | 2026-03-06**

---

## Project State

PyO3-based Python bindings for pdfplumber-rs. Exposes `PyPdf`, `PyPage`, `PyTable`, and `PyCroppedPage` as Python classes. Built with `maturin`. **lib.rs is 1,481 lines and needs splitting.**

### What's Built

All four Python classes fully implemented with `__repr__`, property accessors, and method parity with the Rust API.

### What's Not Done

- **File splitting** (Agent B owns this):
  - `lib.rs` (1,481 lines) → `py_pdf.rs`, `py_page.rs`, `py_table.rs`, `py_cropped.rs`
- Type stubs (`.pyi` files) for IDE autocomplete
- Published to PyPI

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| `pdfplumber` | All extraction APIs |
| `pyo3` | Python FFI |
| `maturin` | Build system (external) |

---

## Architecture Rules

1. **`#![warn(unsafe_code)]` not `#![forbid]`.** PyO3 requires `unsafe` for FFI. Use per-function `#[allow(unsafe_code)]` with safety comments.
2. **Python exceptions from Rust errors.** All `PdfError` variants must map to a Python exception type (use `PyValueError` / `PyIOError` as appropriate — never `unwrap()`).
3. **lib.rs split must preserve module registration.** The `#[pymodule]` macro and `fn pdfplumber_module(m: &Bound<'_, PyModule>)` function must remain in `lib.rs` (or `mod.rs`).

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md. Flagged lib.rs for Agent B splitting.
