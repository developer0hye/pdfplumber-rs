use std::path::Path;

use pdfplumber::{TextOptions, UnicodeNorm};

use crate::cli::TextFormat;
use crate::shared::{ProgressReporter, open_pdf_full, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &TextFormat,
    layout: bool,
    unicode_norm: Option<UnicodeNorm>,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, unicode_norm, password)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());

    let text_options = TextOptions {
        layout,
        ..TextOptions::default()
    };

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let text = page.extract_text(&text_options);

        match format {
            TextFormat::Text => {
                println!("--- Page {} ---", idx + 1);
                println!("{text}");
            }
            TextFormat::Json => {
                let obj = serde_json::json!({
                    "page": idx + 1,
                    "text": text,
                });
                println!("{}", serde_json::to_string(&obj).unwrap());
            }
        }
    }

    progress.finish();
    Ok(())
}
