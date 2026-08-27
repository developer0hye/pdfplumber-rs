//! Extract tables from a PDF document and print them as grids.
//!
//! Usage: `cargo run -p pdfplumber --example extract_table -- <path-to-pdf>`

use pdfplumber::{Pdf, TableSettings};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: extract_table <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(&path, None)?;

    let settings = TableSettings::default();

    for page_result in pdf.pages() {
        let page = page_result?;
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

    Ok(())
}
