use std::path::Path;

use pdfplumber::Annotation;

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, csv_escape, open_pdf, resolve_pages};

pub fn run(file: &Path, pages: Option<&str>, format: &OutputFormat) -> Result<(), i32> {
    let pdf = open_pdf(file)?;
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
    println!("page\ttype\tx0\ttop\tx1\tbottom\tcontents\tauthor\tdate");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for annot in page.annots() {
            println!(
                "{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}",
                idx + 1,
                annot.raw_subtype,
                annot.bbox.x0,
                annot.bbox.top,
                annot.bbox.x1,
                annot.bbox.bottom,
                annot.contents.as_deref().unwrap_or(""),
                annot.author.as_deref().unwrap_or(""),
                annot.date.as_deref().unwrap_or(""),
            );
        }
    }

    progress.finish();
    Ok(())
}

fn annot_to_json(annot: &Annotation, page_num: usize) -> serde_json::Value {
    serde_json::json!({
        "page": page_num,
        "type": annot.raw_subtype,
        "x0": annot.bbox.x0,
        "top": annot.bbox.top,
        "x1": annot.bbox.x1,
        "bottom": annot.bbox.bottom,
        "contents": annot.contents,
        "author": annot.author,
        "date": annot.date,
    })
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_annots = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for annot in page.annots() {
            all_annots.push(annot_to_json(annot, idx + 1));
        }
    }

    let json_str = serde_json::to_string(&all_annots).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,type,x0,top,x1,bottom,contents,author,date");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for annot in page.annots() {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2},{},{},{}",
                idx + 1,
                annot.raw_subtype,
                annot.bbox.x0,
                annot.bbox.top,
                annot.bbox.x1,
                annot.bbox.bottom,
                csv_escape(annot.contents.as_deref().unwrap_or("")),
                csv_escape(annot.author.as_deref().unwrap_or("")),
                csv_escape(annot.date.as_deref().unwrap_or("")),
            );
        }
    }

    progress.finish();
    Ok(())
}
