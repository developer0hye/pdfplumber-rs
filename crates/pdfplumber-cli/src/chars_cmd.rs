use std::path::Path;

use pdfplumber::{Char, UnicodeNorm};

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, direction_str, open_pdf_maybe_repair, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    unicode_norm: Option<UnicodeNorm>,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, unicode_norm, password, repair)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());

    match format {
        OutputFormat::Text => write_text(&pdf, &page_indices, &progress),
        OutputFormat::Json => write_json(&pdf, &page_indices, &progress),
        OutputFormat::Csv => write_csv(&pdf, &page_indices, &progress),
    }
}

fn write_text(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page\ttext\tx0\ttop\tx1\tbottom\tfontname\tsize\tdoctop\tupright\tdirection");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let chars = page.chars();
        for ch in chars {
            println!(
                "{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{:.2}\t{:.2}\t{}\t{}",
                idx + 1,
                ch.text,
                ch.bbox.x0,
                ch.bbox.top,
                ch.bbox.x1,
                ch.bbox.bottom,
                ch.fontname,
                ch.size,
                ch.doctop,
                ch.upright,
                direction_str(&ch.direction),
            );
        }
    }

    progress.finish();
    Ok(())
}

fn char_to_json(ch: &Char, page_num: usize) -> serde_json::Value {
    serde_json::json!({
        "page": page_num,
        "text": ch.text,
        "fontname": ch.fontname,
        "size": ch.size,
        "x0": ch.bbox.x0,
        "top": ch.bbox.top,
        "x1": ch.bbox.x1,
        "bottom": ch.bbox.bottom,
        "doctop": ch.doctop,
        "upright": ch.upright,
        "direction": direction_str(&ch.direction),
    })
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_chars = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let chars = page.chars();
        for ch in chars {
            all_chars.push(char_to_json(ch, idx + 1));
        }
    }

    let json_str = serde_json::to_string(&all_chars).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,text,x0,top,x1,bottom,fontname,size");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let chars = page.chars();
        for ch in chars {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2},{},{:.2}",
                idx + 1,
                ch.text,
                ch.bbox.x0,
                ch.bbox.top,
                ch.bbox.x1,
                ch.bbox.bottom,
                ch.fontname,
                ch.size,
            );
        }
    }

    progress.finish();
    Ok(())
}
