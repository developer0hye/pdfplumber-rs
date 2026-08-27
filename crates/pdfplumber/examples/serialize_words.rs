//! Serialize curated word models using the versioned Serde JSON contract.
//!
//! Usage: `cargo run -p pdfplumber --example serialize_words --features serde -- <path-to-pdf>`

use pdfplumber::{
    Pdf,
    models::{SERDE_JSON_SCHEMA, WordOptions},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: serialize_words <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(path, None)?;
    let first_page = pdf.pages().get(0)?;
    let words = first_page.extract_words(&WordOptions::default());

    println!("schema: {SERDE_JSON_SCHEMA}");
    println!("{}", serde_json::to_string_pretty(&words)?);

    Ok(())
}
