use std::fs;
use std::path::Path;

use crate::cli::OutputFormat;
use crate::shared::{ProgressReporter, open_pdf_full, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    extract: bool,
    output_dir: Option<&Path>,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let reporter = ProgressReporter::new(page_indices.len());

    if extract {
        run_extract(&pdf, &page_indices, output_dir, &reporter)
    } else {
        match format {
            OutputFormat::Text => write_text(&pdf, &page_indices, &reporter),
            OutputFormat::Json => write_json(&pdf, &page_indices, &reporter),
            OutputFormat::Csv => write_csv(&pdf, &page_indices, &reporter),
        }
    }
}

fn write_text(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    reporter: &ProgressReporter,
) -> Result<(), i32> {
    for (i, &idx) in page_indices.iter().enumerate() {
        reporter.report(i + 1);
        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        println!("--- Page {} ---", idx + 1);
        for img in page.images() {
            println!(
                "  {}\tx0={:.1}\ttop={:.1}\tx1={:.1}\tbottom={:.1}\t{}x{}\t{}",
                img.name,
                img.x0,
                img.top,
                img.x1,
                img.bottom,
                img.src_width.map_or("-".to_string(), |w| w.to_string()),
                img.src_height.map_or("-".to_string(), |h| h.to_string()),
                img.color_space.as_deref().unwrap_or("-"),
            );
        }
    }
    reporter.finish();
    Ok(())
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    reporter: &ProgressReporter,
) -> Result<(), i32> {
    let mut pages_json = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        reporter.report(i + 1);
        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let images_json: Vec<serde_json::Value> = page
            .images()
            .iter()
            .map(|img| {
                serde_json::json!({
                    "name": img.name,
                    "x0": img.x0,
                    "top": img.top,
                    "x1": img.x1,
                    "bottom": img.bottom,
                    "width": img.width,
                    "height": img.height,
                    "src_width": img.src_width,
                    "src_height": img.src_height,
                    "bits_per_component": img.bits_per_component,
                    "color_space": img.color_space,
                })
            })
            .collect();

        pages_json.push(serde_json::json!({
            "page": idx + 1,
            "images": images_json,
        }));
    }
    reporter.finish();

    let output = serde_json::to_string_pretty(&pages_json).unwrap();
    println!("{output}");
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    reporter: &ProgressReporter,
) -> Result<(), i32> {
    println!(
        "page,name,x0,top,x1,bottom,width,height,src_width,src_height,bits_per_component,color_space"
    );

    for (i, &idx) in page_indices.iter().enumerate() {
        reporter.report(i + 1);
        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        for img in page.images() {
            println!(
                "{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{},{}",
                idx + 1,
                img.name,
                img.x0,
                img.top,
                img.x1,
                img.bottom,
                img.width,
                img.height,
                img.src_width.map_or(String::new(), |w| w.to_string()),
                img.src_height.map_or(String::new(), |h| h.to_string()),
                img.bits_per_component
                    .map_or(String::new(), |b| b.to_string()),
                img.color_space.as_deref().unwrap_or(""),
            );
        }
    }
    reporter.finish();
    Ok(())
}

fn run_extract(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    output_dir: Option<&Path>,
    reporter: &ProgressReporter,
) -> Result<(), i32> {
    let dir = output_dir.unwrap_or(Path::new("."));

    // Create output directory if needed
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| {
            eprintln!("Error creating directory {}: {e}", dir.display());
            1
        })?;
    }

    let mut count = 0;
    for (i, &idx) in page_indices.iter().enumerate() {
        reporter.report(i + 1);

        let pairs = pdf.extract_images_with_content(idx).map_err(|e| {
            eprintln!("Error extracting images from page {}: {e}", idx + 1);
            1
        })?;

        for (image, content) in &pairs {
            let ext = content.format.extension();
            let filename = format!("page{}_{}.{}", idx + 1, image.name, ext);
            let path = dir.join(&filename);

            fs::write(&path, &content.data).map_err(|e| {
                eprintln!("Error writing {}: {e}", path.display());
                1
            })?;

            eprintln!("Wrote {} ({} bytes)", path.display(), content.data.len());
            count += 1;
        }
    }
    reporter.finish();

    if count == 0 {
        eprintln!("No images found.");
    } else {
        eprintln!("Extracted {count} image(s).");
    }

    Ok(())
}
