use std::fs;
use std::path::Path;

use pdfplumber::{DrawStyle, SvgOptions, SvgRenderer, TableSettings};

use crate::shared::{open_pdf, resolve_pages};

pub fn run(file: &Path, pages: Option<&str>, output: &Path) -> Result<(), i32> {
    let pdf = open_pdf(file)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;

    // Generate SVG for each page; if multiple pages, append page number to filename
    let multi_page = page_indices.len() > 1;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("debug");
    let ext = output.extension().and_then(|s| s.to_str()).unwrap_or("svg");
    let parent = output.parent().unwrap_or(Path::new("."));

    for &idx in &page_indices {
        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let mut renderer = SvgRenderer::new(page.width(), page.height());

        // Draw all extracted objects with default styles
        renderer.draw_chars(page.chars(), &DrawStyle::chars_default());
        renderer.draw_lines(page.lines(), &DrawStyle::lines_default());
        renderer.draw_rects(page.rects(), &DrawStyle::rects_default());

        // Draw edges
        let edges = page.edges();
        renderer.draw_edges(&edges, &DrawStyle::edges_default());

        // Draw tables
        let tables = page.find_tables(&TableSettings::default());
        renderer.draw_tables(&tables, &DrawStyle::tables_default());

        let svg = renderer.to_svg(&SvgOptions::default());

        let out_path = if multi_page {
            parent.join(format!("{stem}_page{}.{ext}", idx + 1))
        } else {
            output.to_path_buf()
        };

        fs::write(&out_path, &svg).map_err(|e| {
            eprintln!("Error writing {}: {e}", out_path.display());
            1
        })?;

        eprintln!("Wrote {}", out_path.display());
    }

    Ok(())
}
