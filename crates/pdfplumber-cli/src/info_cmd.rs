use std::path::Path;

use pdfplumber::{BBox, TableSettings};

use crate::cli::TextFormat;
use crate::shared::{ProgressReporter, open_pdf_full, resolve_pages};

fn format_bbox(b: &BBox) -> String {
    format!("[{:.2}, {:.2}, {:.2}, {:.2}]", b.x0, b.top, b.x1, b.bottom)
}

fn bbox_to_json(b: &BBox) -> serde_json::Value {
    serde_json::json!([b.x0, b.top, b.x1, b.bottom])
}

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &TextFormat,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;
    let page_count = pdf.page_count();
    let page_indices = resolve_pages(pages, page_count)?;
    let progress = ProgressReporter::new(page_indices.len());
    let metadata = pdf.metadata();

    let settings = TableSettings::default();

    let mut total_chars: usize = 0;
    let mut total_tables: usize = 0;
    let mut page_infos: Vec<serde_json::Value> = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let chars_count = page.chars().len();
        let lines_count = page.lines().len();
        let rects_count = page.rects().len();
        let curves_count = page.curves().len();
        let images_count = page.images().len();
        let tables_count = page.find_tables(&settings).len();

        total_chars += chars_count;
        total_tables += tables_count;

        match format {
            TextFormat::Text => {
                println!("Page {}:", idx + 1);
                println!("  Dimensions: {:.2} x {:.2}", page.width(), page.height());
                println!("  Rotation: {}°", page.rotation());
                println!("  MediaBox: {}", format_bbox(&page.media_box()));
                if let Some(ref cb) = page.crop_box() {
                    println!("  CropBox: {}", format_bbox(cb));
                }
                if let Some(ref tb) = page.trim_box() {
                    println!("  TrimBox: {}", format_bbox(tb));
                }
                if let Some(ref bb) = page.bleed_box() {
                    println!("  BleedBox: {}", format_bbox(bb));
                }
                if let Some(ref ab) = page.art_box() {
                    println!("  ArtBox: {}", format_bbox(ab));
                }
                println!("  Chars: {chars_count}");
                println!("  Lines: {lines_count}");
                println!("  Rects: {rects_count}");
                println!("  Curves: {curves_count}");
                println!("  Images: {images_count}");
            }
            TextFormat::Json => {
                let mut page_json = serde_json::json!({
                    "page": idx + 1,
                    "width": page.width(),
                    "height": page.height(),
                    "rotation": page.rotation(),
                    "media_box": bbox_to_json(&page.media_box()),
                    "chars": chars_count,
                    "lines": lines_count,
                    "rects": rects_count,
                    "curves": curves_count,
                    "images": images_count,
                });
                if let Some(ref cb) = page.crop_box() {
                    page_json["crop_box"] = bbox_to_json(cb);
                }
                if let Some(ref tb) = page.trim_box() {
                    page_json["trim_box"] = bbox_to_json(tb);
                }
                if let Some(ref bb) = page.bleed_box() {
                    page_json["bleed_box"] = bbox_to_json(bb);
                }
                if let Some(ref ab) = page.art_box() {
                    page_json["art_box"] = bbox_to_json(ab);
                }
                page_infos.push(page_json);
            }
        }
    }

    progress.finish();

    match format {
        TextFormat::Text => {
            if !metadata.is_empty() {
                println!();
                println!("Metadata:");
                if let Some(ref v) = metadata.title {
                    println!("  Title: {v}");
                }
                if let Some(ref v) = metadata.author {
                    println!("  Author: {v}");
                }
                if let Some(ref v) = metadata.subject {
                    println!("  Subject: {v}");
                }
                if let Some(ref v) = metadata.keywords {
                    println!("  Keywords: {v}");
                }
                if let Some(ref v) = metadata.creator {
                    println!("  Creator: {v}");
                }
                if let Some(ref v) = metadata.producer {
                    println!("  Producer: {v}");
                }
                if let Some(ref v) = metadata.creation_date {
                    println!("  CreationDate: {v}");
                }
                if let Some(ref v) = metadata.mod_date {
                    println!("  ModDate: {v}");
                }
            }
            println!();
            println!("Pages: {page_count}");
            println!();
            println!("Summary:");
            println!("  Total chars: {total_chars}");
            println!("  Total tables: {total_tables}");
        }
        TextFormat::Json => {
            let mut metadata_json = serde_json::Map::new();
            if let Some(ref v) = metadata.title {
                metadata_json.insert("title".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.author {
                metadata_json.insert("author".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.subject {
                metadata_json.insert("subject".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.keywords {
                metadata_json.insert("keywords".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.creator {
                metadata_json.insert("creator".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.producer {
                metadata_json.insert("producer".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.creation_date {
                metadata_json.insert("creation_date".to_string(), serde_json::json!(v));
            }
            if let Some(ref v) = metadata.mod_date {
                metadata_json.insert("mod_date".to_string(), serde_json::json!(v));
            }

            let output = serde_json::json!({
                "metadata": metadata_json,
                "pages": page_count,
                "page_info": page_infos,
                "summary": {
                    "total_chars": total_chars,
                    "total_tables": total_tables,
                },
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    Ok(())
}
