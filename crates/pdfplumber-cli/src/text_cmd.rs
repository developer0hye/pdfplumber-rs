use std::path::Path;

use pdfplumber::{HtmlOptions, TextOptions, UnicodeNorm};

use crate::cli::TextFormat;
use crate::shared::{ProgressReporter, open_pdf_maybe_repair, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &TextFormat,
    layout: bool,
    unicode_norm: Option<UnicodeNorm>,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, unicode_norm, password, repair)?;
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

        match format {
            TextFormat::Text => {
                let text = page.extract_text(&text_options);
                println!("--- Page {} ---", idx + 1);
                println!("{text}");
            }
            TextFormat::Json => {
                let text = page.extract_text(&text_options);
                let obj = serde_json::json!({
                    "page": idx + 1,
                    "text": text,
                });
                println!("{}", serde_json::to_string(&obj).unwrap());
            }
            TextFormat::Html => {
                let html = page.to_html(&HtmlOptions::default());
                println!("--- Page {} ---", idx + 1);
                println!("{html}");
            }
        }
    }

    progress.finish();
    Ok(())
}
