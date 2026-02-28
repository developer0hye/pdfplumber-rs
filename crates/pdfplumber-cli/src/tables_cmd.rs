use std::path::Path;

use pdfplumber::{Strategy, TableSettings};

use crate::cli::{OutputFormat, TableStrategy};
use crate::shared::{ProgressReporter, csv_escape, open_pdf_maybe_repair, resolve_pages};

#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    pages: Option<&str>,
    format: &OutputFormat,
    strategy: &TableStrategy,
    snap_tolerance: f64,
    join_tolerance: f64,
    text_tolerance: f64,
    password: Option<&str>,
    repair: bool,
) -> Result<(), i32> {
    let pdf = open_pdf_maybe_repair(file, None, password, repair)?;
    let page_indices = resolve_pages(pages, pdf.page_count())?;
    let progress = ProgressReporter::new(page_indices.len());

    let settings = build_settings(strategy, snap_tolerance, join_tolerance, text_tolerance);

    match format {
        OutputFormat::Text => write_grid(&pdf, &page_indices, &settings, &progress),
        OutputFormat::Json => write_json(&pdf, &page_indices, &settings, &progress),
        OutputFormat::Csv => write_csv(&pdf, &page_indices, &settings, &progress),
    }
}

fn build_settings(
    strategy: &TableStrategy,
    snap_tolerance: f64,
    join_tolerance: f64,
    text_tolerance: f64,
) -> TableSettings {
    let core_strategy = match strategy {
        TableStrategy::Lattice => Strategy::Lattice,
        TableStrategy::Stream => Strategy::Stream,
    };

    TableSettings {
        strategy: core_strategy,
        snap_tolerance,
        snap_x_tolerance: snap_tolerance,
        snap_y_tolerance: snap_tolerance,
        join_tolerance,
        join_x_tolerance: join_tolerance,
        join_y_tolerance: join_tolerance,
        text_tolerance,
        text_x_tolerance: text_tolerance,
        text_y_tolerance: text_tolerance,
        ..TableSettings::default()
    }
}

fn write_grid(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    settings: &TableSettings,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut table_num = 0;

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let tables = page.find_tables(settings);

        for table in &tables {
            table_num += 1;
            println!(
                "--- Table {} (page {}, bbox: [{:.1}, {:.1}, {:.1}, {:.1}]) ---",
                table_num,
                idx + 1,
                table.bbox.x0,
                table.bbox.top,
                table.bbox.x1,
                table.bbox.bottom,
            );

            if table.rows.is_empty() {
                continue;
            }

            // Compute column widths for aligned output
            let col_count = table.rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut col_widths = vec![0usize; col_count];

            let text_rows: Vec<Vec<String>> = table
                .rows
                .iter()
                .map(|row| {
                    let mut texts = Vec::new();
                    for (ci, cell) in row.iter().enumerate() {
                        let text = cell.text.as_deref().unwrap_or("");
                        if ci < col_widths.len() {
                            col_widths[ci] = col_widths[ci].max(text.len());
                        }
                        texts.push(text.to_string());
                    }
                    // Pad if this row has fewer columns
                    while texts.len() < col_count {
                        texts.push(String::new());
                    }
                    texts
                })
                .collect();

            // Ensure minimum width of 1
            for w in &mut col_widths {
                if *w == 0 {
                    *w = 1;
                }
            }

            // Print rows with | separators
            for row_texts in &text_rows {
                let cells_formatted: Vec<String> = row_texts
                    .iter()
                    .enumerate()
                    .map(|(ci, text)| {
                        let width = col_widths.get(ci).copied().unwrap_or(1);
                        format!("{:<width$}", text, width = width)
                    })
                    .collect();
                println!("| {} |", cells_formatted.join(" | "));
            }

            println!();
        }
    }

    if table_num == 0 {
        println!("No tables found.");
    }

    progress.finish();
    Ok(())
}

fn write_json(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    settings: &TableSettings,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut all_tables = Vec::new();

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let tables = page.find_tables(settings);

        for table in &tables {
            let rows: Vec<Vec<Option<&str>>> = table
                .rows
                .iter()
                .map(|row| row.iter().map(|cell| cell.text.as_deref()).collect())
                .collect();

            let quality = table.quality();
            all_tables.push(serde_json::json!({
                "page": idx + 1,
                "bbox": {
                    "x0": table.bbox.x0,
                    "top": table.bbox.top,
                    "x1": table.bbox.x1,
                    "bottom": table.bbox.bottom,
                },
                "rows": rows,
                "quality": {
                    "accuracy": quality.accuracy,
                    "whitespace": quality.whitespace,
                },
            }));
        }
    }

    let json_str = serde_json::to_string(&all_tables).unwrap();
    println!("{json_str}");

    progress.finish();
    Ok(())
}

fn write_csv(
    pdf: &pdfplumber::Pdf,
    page_indices: &[usize],
    settings: &TableSettings,
    progress: &ProgressReporter,
) -> Result<(), i32> {
    let mut first_table = true;

    for (i, &idx) in page_indices.iter().enumerate() {
        progress.report(i + 1);

        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let tables = page.find_tables(settings);

        for table in &tables {
            if !first_table {
                println!();
            }
            first_table = false;

            for row in &table.rows {
                let cells: Vec<String> = row
                    .iter()
                    .map(|cell| csv_escape(cell.text.as_deref().unwrap_or("")))
                    .collect();
                println!("{}", cells.join(","));
            }
        }
    }

    progress.finish();
    Ok(())
}
