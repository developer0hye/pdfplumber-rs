use std::path::Path;

use pdfplumber::{UnicodeNorm, WordOptions};

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, direction_str, open_pdf_full, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    x_tolerance: f64,
    y_tolerance: f64,
    unicode_norm: Option<UnicodeNorm>,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, unicode_norm, password)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());

    let opts = WordOptions {
        x_tolerance,
        y_tolerance,
        ..WordOptions::default()
    };

    match format {
        OutputFormat::Text => write_text(&pdf, &page_indices, &opts, &progress),
        OutputFormat::Json => write_json(&pdf, &page_indices, &opts, &progress),
        OutputFormat::Csv => write_csv(&pdf, &page_indices, &opts, &progress),
    }
}

fn write_text(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    opts: &WordOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page\ttext\tx0\ttop\tx1\tbottom\tdoctop\tdirection");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let words = page.extract_words(opts);
        for w in &words {
            println!(
                "{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}",
                idx + 1,
                w.text,
                w.bbox.x0,
                w.bbox.top,
                w.bbox.x1,
                w.bbox.bottom,
                w.doctop,
                direction_str(&w.direction),
            );
        }
    }

    progress.finish();
    Ok(())
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    opts: &WordOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_words = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let words = page.extract_words(opts);
        for w in &words {
            all_words.push(serde_json::json!({
                "page": idx + 1,
                "text": w.text,
                "x0": w.bbox.x0,
                "top": w.bbox.top,
                "x1": w.bbox.x1,
                "bottom": w.bbox.bottom,
                "doctop": w.doctop,
                "direction": direction_str(&w.direction),
            }));
        }
    }

    let json_str = serde_json::to_string(&all_words).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    opts: &WordOptions,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,text,x0,top,x1,bottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let words = page.extract_words(opts);
        for w in &words {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2}",
                idx + 1,
                w.text,
                w.bbox.x0,
                w.bbox.top,
                w.bbox.x1,
                w.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}
