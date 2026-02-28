use std::path::Path;

use pdfplumber::{Char, Pdf, TextDirection};

use crate::cli::OutputFormat;
use crate::page_range::parse_page_range;

pub fn run(file: &Path, pages: Option<&str>, format: &OutputFormat) -> Result<(), i32> {
    let pdf = open_pdf(file)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;

    match format {
        OutputFormat::Text => write_text(&pdf, &page_indices),
        OutputFormat::Json => write_json(&pdf, &page_indices),
        OutputFormat::Csv => write_csv(&pdf, &page_indices),
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

fn write_text(pdf: &Pdf, page_indices: &[usize]) -> Result<(), i32> {
    println!("page\ttext\tx0\ttop\tx1\tbottom\tfontname\tsize\tdoctop\tupright\tdirection");

    for &idx in page_indices {
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

fn write_json(pdf: &Pdf, page_indices: &[usize]) -> Result<(), i32> {
    let mut all_chars = Vec::new();

    for &idx in page_indices {
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

    Ok(())
}

fn write_csv(pdf: &Pdf, page_indices: &[usize]) -> Result<(), i32> {
    println!("page,text,x0,top,x1,bottom,fontname,size");

    for &idx in page_indices {
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

    Ok(())
}
