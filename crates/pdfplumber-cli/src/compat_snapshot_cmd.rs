use std::path::Path;

use pdfplumber::TextOptions;

use crate::shared::{ProgressReporter, open_pdf_maybe_repair, resolve_pages};

/// Emit the Rust API values that have no standalone public CLI command.
///
/// This command is hidden from normal CLI help because its JSON is an internal
/// compatibility-harness transport, not a stable public serialization format.
pub fn run(
    file: &Path,
    pages: Option<&str>,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, None, password, repair)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());
    let mut snapshots = Vec::with_capacity(page_indices.len());

    for (position, &page_index) in page_indices.iter().enumerate() {
        progress.report(position + 1);
        let page = pdf.page(page_index).map_err(|error| {
            eprintln!("Error reading page {}: {error}", page_index + 1);
            1
        })?;
        let text_lines = page.extract_text_lines(&TextOptions::default());
        let structure_tree = page.structure_tree().unwrap_or(&[]);

        snapshots.push(serde_json::json!({
            "page": page_index + 1,
            "text_lines": text_lines,
            "structure_tree": structure_tree,
        }));
    }

    println!("{}", serde_json::to_string(&snapshots).unwrap());
    progress.finish();
    Ok(())
}
