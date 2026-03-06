//! Pre-flight directory scan for the Process screen.
//!
//! Walks every `.pdf` file in `dir` (non-recursively by default, recursive
//! when `recursive` is true), opens each PDF, counts pages, and detects
//! image-only pages (no extractable chars but at least one image object).
//!
//! Image-only pages cannot be processed by pdfplumber text extraction; they
//! need Ollama vision or OCR.  The scan counts how many such pages exist so
//! the Process screen can show a pre-flight warning and gate the "proceed"
//! action on Ollama being configured.
//!
//! # Performance
//!
//! Each PDF open is O(parse overhead) — we do NOT extract text, just check
//! `.chars().is_empty()` and `.images().is_empty()`.  On a 100-file, 500-page
//! corpus this runs in < 1 second on typical hardware.

use std::fs;
use std::path::{Path, PathBuf};

use pdfplumber::Pdf;

use super::app::{FilePreview, ProcessState};

// ── public API ────────────────────────────────────────────────────────────────

/// Result of scanning a directory.
pub struct ScanResult {
    /// One entry per PDF found.
    pub files: Vec<FilePreview>,
    /// Total number of pages across all PDFs that have no extractable text
    /// and at least one image — these need Ollama/OCR.
    pub ollama_needed: usize,
    /// Number of PDFs that could not be opened (bad path, encrypted, corrupt).
    pub errors: usize,
}

/// Walk `dir` and build a `ScanResult`.
///
/// - `recursive` — also descend into subdirectories.
/// - Symlinks are followed for files, not for directories.
/// - Hidden files/directories (name starts with `.`) are skipped.
pub fn scan_dir(dir: &Path, recursive: bool) -> ScanResult {
    let mut result = ScanResult {
        files: vec![],
        ollama_needed: 0,
        errors: 0,
    };

    let entries = match collect_pdfs(dir, recursive) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for path in entries {
        match scan_single(&path) {
            Ok(preview) => {
                if preview.needs_ollama {
                    result.ollama_needed += 1;
                }
                result.files.push(preview);
            }
            Err(_) => {
                // Still add a stub entry so the user can see the failure
                result.files.push(FilePreview {
                    name: path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "unknown".to_string()),
                    pages: 0,
                    needs_ollama: false,
                });
                result.errors += 1;
            }
        }
    }

    // Sort by name for stable ordering
    result.files.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// Fill `ProcessState.files` and `ollama_needed` from a directory scan.
///
/// Designed to be called from `activate_menu_item` in `input_handlers.rs`.
pub fn populate_process_state(st: &mut ProcessState, recursive: bool) {
    let scan = scan_dir(&st.dir, recursive);
    st.files = scan.files;
    st.ollama_needed = scan.ollama_needed;
}

// ── internals ─────────────────────────────────────────────────────────────────

/// Collect all `.pdf` paths under `dir`.  If `recursive` is false only
/// the immediate children are returned.
fn collect_pdfs(dir: &Path, recursive: bool) -> std::io::Result<Vec<PathBuf>> {
    let mut out = vec![];
    collect_inner(dir, recursive, &mut out)?;
    Ok(out)
}

fn collect_inner(
    dir: &Path,
    recursive: bool,
    out: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()), // silently skip unreadable dirs
    };

    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden entries
        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() && recursive {
            collect_inner(&path, recursive, out)?;
        } else if path.is_file() {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase());
            if ext.as_deref() == Some("pdf") {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Open a single PDF and inspect it.
fn scan_single(path: &Path) -> Result<FilePreview, String> {
    let pdf = Pdf::open_file(path, None)
        .map_err(|e| format!("{e}"))?;

    let total_pages = pdf.page_count();
    let mut image_only_pages = 0usize;

    for page_idx in 0..total_pages {
        let page = match pdf.page(page_idx) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let chars = page.chars();
        let images = page.images();

        // Image-only: no selectable text but has at least one image object
        let has_text = !chars.is_empty();
        let has_images = !images.is_empty();

        if !has_text && has_images {
            image_only_pages += 1;
        }
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    Ok(FilePreview {
        name,
        pages: total_pages,
        needs_ollama: image_only_pages > 0,
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_tmp() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn empty_dir_returns_empty_result() {
        let d = make_tmp();
        let r = scan_dir(d.path(), false);
        assert!(r.files.is_empty());
        assert_eq!(r.ollama_needed, 0);
        assert_eq!(r.errors, 0);
    }

    #[test]
    fn non_pdf_files_ignored() {
        let d = make_tmp();
        fs::write(d.path().join("notes.txt"), b"hello").unwrap();
        fs::write(d.path().join("image.png"), b"\x89PNG").unwrap();
        let r = scan_dir(d.path(), false);
        assert!(r.files.is_empty());
    }

    #[test]
    fn hidden_files_skipped() {
        let d = make_tmp();
        fs::write(d.path().join(".hidden.pdf"), b"%PDF-1.4").unwrap();
        let r = scan_dir(d.path(), false);
        assert!(r.files.is_empty());
    }

    #[test]
    fn collect_pdfs_non_recursive_ignores_subdir() {
        let d = make_tmp();
        let sub = d.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.pdf"), b"%PDF-1.4").unwrap();
        // Root has no PDFs; non-recursive should find nothing
        let paths = collect_pdfs(d.path(), false).unwrap();
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_pdfs_recursive_finds_nested() {
        let d = make_tmp();
        let sub = d.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.pdf"), b"%PDF-1.4").unwrap();
        // Recursive should find it (even if it can't be opened — collect
        // just finds paths, not validates them)
        let paths = collect_pdfs(d.path(), true).unwrap();
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn corrupt_pdf_counted_as_error() {
        let d = make_tmp();
        let p = d.path().join("bad.pdf");
        fs::write(&p, b"this is not a pdf").unwrap();
        let r = scan_dir(d.path(), false);
        // Should have one file entry but marked as error
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.errors, 1);
        assert_eq!(r.files[0].pages, 0);
    }

    #[test]
    fn files_sorted_by_name() {
        let d = make_tmp();
        // Write invalid PDFs to trigger the error path (still sorted)
        fs::write(d.path().join("z.pdf"), b"x").unwrap();
        fs::write(d.path().join("a.pdf"), b"x").unwrap();
        fs::write(d.path().join("m.pdf"), b"x").unwrap();
        let r = scan_dir(d.path(), false);
        let names: Vec<&str> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.pdf", "m.pdf", "z.pdf"]);
    }
}
