use std::path::Path;

use pdfplumber::Hyperlink;

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, csv_escape, open_pdf_maybe_repair, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, None, password, repair)?;
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
    println!("page\turi\tx0\ttop\tx1\tbottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for link in page.hyperlinks() {
            println!(
                "{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
                idx + 1,
                link.uri,
                link.bbox.x0,
                link.bbox.top,
                link.bbox.x1,
                link.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}

fn link_to_json(link: &Hyperlink, page_num: usize) -> serde_json::Value {
    serde_json::json!({
        "page": page_num,
        "uri": link.uri,
        "x0": link.bbox.x0,
        "top": link.bbox.top,
        "x1": link.bbox.x1,
        "bottom": link.bbox.bottom,
    })
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_links = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for link in page.hyperlinks() {
            all_links.push(link_to_json(link, idx + 1));
        }
    }

    let json_str = serde_json::to_string(&all_links).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,uri,x0,top,x1,bottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for link in page.hyperlinks() {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2}",
                idx + 1,
                csv_escape(&link.uri),
                link.bbox.x0,
                link.bbox.top,
                link.bbox.x1,
                link.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}
