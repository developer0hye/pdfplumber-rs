use std::path::Path;

use pdfplumber::Bookmark;

use crate::cli::TextFormat;
use crate::shared::open_pdf_full;

pub fn run(file: &Path, format: &TextFormat, password: Option<&str>) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;
    let bookmarks = pdf.bookmarks();

    match format {
        TextFormat::Text | TextFormat::Markdown | TextFormat::Html => write_text(bookmarks),
        TextFormat::Json => write_json(bookmarks),
    }
}

fn write_text(bookmarks: &[Bookmark]) -> Result<(), i32> {
    if bookmarks.is_empty() {
        println!("No bookmarks found.");
        return Ok(());
    }

    println!("level\tpage\ttitle");

    for bm in bookmarks {
        let indent = "  ".repeat(bm.level);
        let page_str = match bm.page_number {
            Some(p) => (p + 1).to_string(),
            None => "-".to_string(),
        };
        println!("{}\t{}\t{}{}", bm.level, page_str, indent, bm.title);
    }

    Ok(())
}

fn write_json(bookmarks: &[Bookmark]) -> Result<(), i32> {
    let json_values: Vec<serde_json::Value> = bookmarks
        .iter()
        .map(|bm| {
            let mut obj = serde_json::json!({
                "title": bm.title,
                "level": bm.level,
            });
            if let Some(page) = bm.page_number {
                obj["page_number"] = serde_json::json!(page);
            }
            if let Some(top) = bm.dest_top {
                obj["dest_top"] = serde_json::json!(top);
            }
            obj
        })
        .collect();

    let json_str = serde_json::to_string(&json_values).unwrap();
    println!("{json_str}");

    Ok(())
}
