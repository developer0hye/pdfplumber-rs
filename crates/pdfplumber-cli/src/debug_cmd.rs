use std::fs;
use std::path::Path;

use pdfplumber::{DrawStyle, SvgDebugOptions, SvgOptions, SvgRenderer, TableSettings};

use crate::shared::{open_pdf_maybe_repair, resolve_pages};

pub fn run(
    file: &Path,
    pages: Option<&str>,
    output: &Path,
    tables: bool,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, None, password, repair)?;
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

        let svg = if tables {
            // Table detection debug mode: show pipeline stages
            page.debug_tablefinder_svg(&TableSettings::default(), &SvgDebugOptions::default())
        } else {
            // Standard debug mode: show all extracted objects
            let mut renderer = SvgRenderer::new(page.width(), page.height());

            renderer.draw_chars(page.chars(), &DrawStyle::chars_default());
            renderer.draw_lines(page.lines(), &DrawStyle::lines_default());
            renderer.draw_rects(page.rects(), &DrawStyle::rects_default());

            let edges = page.edges();
            renderer.draw_edges(&edges, &DrawStyle::edges_default());

            let found_tables = page.find_tables(&TableSettings::default());
            renderer.draw_tables(&found_tables, &DrawStyle::tables_default());

            renderer.to_svg(&SvgOptions::default())
        };

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
