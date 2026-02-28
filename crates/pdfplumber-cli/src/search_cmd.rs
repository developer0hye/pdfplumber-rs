use std::path::Path;

use pdfplumber::SearchOptions;

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, csv_escape, open_pdf_full, resolve_pages};

pub fn run(
    file: &Path,
    pattern: &str,
    pages: Option<&str>,
    case_insensitive: bool,
    no_regex: bool,
    format: &OutputFormat,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());

    let opts = SearchOptions {
        regex: !no_regex,
        case_sensitive: !case_insensitive,
    };

    match format {
        OutputFormat::Text => write_text(&pdf, &page_indices, pattern, &opts, &progress),
        OutputFormat::Json => write_json(&pdf, &page_indices, pattern, &opts, &progress),
        OutputFormat::Csv => write_csv(&pdf, &page_indices, pattern, &opts, &progress),
    }
}

fn write_text(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    pattern: &str,
    opts: &SearchOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page\ttext\tx0\ttop\tx1\tbottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let matches = page.search(pattern, opts);
        for m in &matches {
            println!(
                "{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
                idx + 1,
                m.text,
                m.bbox.x0,
                m.bbox.top,
                m.bbox.x1,
                m.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    pattern: &str,
    opts: &SearchOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_matches = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let matches = page.search(pattern, opts);
        for m in &matches {
            all_matches.push(serde_json::json!({
                "page": idx + 1,
                "text": m.text,
                "x0": m.bbox.x0,
                "top": m.bbox.top,
                "x1": m.bbox.x1,
                "bottom": m.bbox.bottom,
                "char_indices": m.char_indices,
            }));
        }
    }

    let json_str = serde_json::to_string(&all_matches).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    pattern: &str,
    opts: &SearchOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,text,x0,top,x1,bottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let matches = page.search(pattern, opts);
        for m in &matches {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2}",
                idx + 1,
                csv_escape(&m.text),
                m.bbox.x0,
                m.bbox.top,
                m.bbox.x1,
                m.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}
