//! Extract tables from a PDF document and print them as grids.
//!
//! Usage: `cargo run --example extract_table -- <path-to-pdf>`

use pdfplumber::{Pdf, TableSettings};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: extract_table <path-to-pdf>");
        std::process::exit(1);
    });

    let pdf = Pdf::open_file(&path, None).unwrap_or_else(|e| {
        eprintln!("Error opening PDF: {e}");
        std::process::exit(1);
    });

    let settings = TableSettings::default();

    for page_result in pdf.pages_iter() {
        let page = page_result.unwrap();
        let tables = page.find_tables(&settings);

        if tables.is_empty() {
            continue;
        }

        println!(
            "--- Page {} ({} table(s)) ---",
            page.page_number(),
            tables.len()
        );

        for (i, table) in tables.iter().enumerate() {
            println!("  Table {}:", i + 1);
            for row in &table.rows {
                let cells: Vec<&str> = row
                    .iter()
                    .map(|c| c.text.as_deref().unwrap_or(""))
                    .collect();
                println!("    {:?}", cells);
            }
            println!();
        }
    }
}
