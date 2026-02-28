use std::path::Path;

use pdfplumber::{Pdf, TextDirection, WordOptions};

use crate::cli::OutputFormat;
use crate::page_range::parse_page_range;

pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    x_tolerance: f64,
    y_tolerance: f64,
) -> Result<(), i32> {
    let pdf = open_pdf(file)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;

    let opts = WordOptions {
        x_tolerance,
        y_tolerance,
        ..WordOptions::default()
    };

    match format {
        OutputFormat::Text => write_text(&pdf, &page_indices, &opts),
        OutputFormat::Json => write_json(&pdf, &page_indices, &opts),
        OutputFormat::Csv => write_csv(&pdf, &page_indices, &opts),
    }
}

fn open_pdf(file: &Path) -> Result<Pdf, i32> {
    if !file.exists() {
        eprintln!("Error: file not found: {}", file.display());
        return Err(1);
    }

    Pdf::open_file(file, None).map_err(|e| {
        eprintln!("Error: failed to open PDF: {e}");
        1
    })
}

fn resolve_pages(pages: Option<&str>, page_count: usize) -> Result<Vec<usize>, i32> {
    match pages {
        Some(range) => parse_page_range(range, page_count).map_err(|e| {
            eprintln!("Error: {e}");
            1
        }),
        None => Ok((0..page_count).collect()),
    }
}

fn direction_str(dir: &TextDirection) -> &'static str {
    match dir {
        TextDirection::Ltr => "ltr",
        TextDirection::Rtl => "rtl",
        TextDirection::Ttb => "ttb",
        TextDirection::Btt => "btt",
    }
}

fn write_text(pdf: &Pdf, page_indices: &[usize], opts: &WordOptions) -> Result<(), i32> {
    println!("page\ttext\tx0\ttop\tx1\tbottom\tdoctop\tdirection");

    for &idx in page_indices {
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

    Ok(())
}

fn write_json(pdf: &Pdf, page_indices: &[usize], opts: &WordOptions) -> Result<(), i32> {
    let mut all_words = Vec::new();

    for &idx in page_indices {
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

    Ok(())
}

fn write_csv(pdf: &Pdf, page_indices: &[usize], opts: &WordOptions) -> Result<(), i32> {
    println!("page,text,x0,top,x1,bottom");

    for &idx in page_indices {
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

    Ok(())
}
