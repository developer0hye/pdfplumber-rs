use std::path::Path;

use pdfplumber::FormField;

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, csv_escape, open_pdf_full, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;
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
    println!("page\tname\ttype\tvalue\tdefault_value\toptions\tflags\tx0\ttop\tx1\tbottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for field in page.form_fields() {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
                idx + 1,
                field.name,
                field.field_type,
                field.value.as_deref().unwrap_or(""),
                field.default_value.as_deref().unwrap_or(""),
                field.options.join("; "),
                field.flags,
                field.bbox.x0,
                field.bbox.top,
                field.bbox.x1,
                field.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}

fn field_to_json(field: &FormField, page_num: usize) -> serde_json::Value {
    serde_json::json!({
        "page": page_num,
        "name": field.name,
        "type": field.field_type.to_string(),
        "value": field.value,
        "default_value": field.default_value,
        "options": field.options,
        "flags": field.flags,
        "x0": field.bbox.x0,
        "top": field.bbox.top,
        "x1": field.bbox.x1,
        "bottom": field.bbox.bottom,
    })
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_fields = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for field in page.form_fields() {
            all_fields.push(field_to_json(field, idx + 1));
        }
    }

    let json_str = serde_json::to_string(&all_fields).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    progress: &ProgressReporter,
) -> Result<(), i32> {
    println!("page,name,type,value,default_value,options,flags,x0,top,x1,bottom");

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for field in page.form_fields() {
            println!(
                "{},{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2}",
                idx + 1,
                csv_escape(&field.name),
                field.field_type,
                csv_escape(field.value.as_deref().unwrap_or("")),
                csv_escape(field.default_value.as_deref().unwrap_or("")),
                csv_escape(&field.options.join("; ")),
                field.flags,
                field.bbox.x0,
                field.bbox.top,
                field.bbox.x1,
                field.bbox.bottom,
            );
        }
    }

    progress.finish();
    Ok(())
}
