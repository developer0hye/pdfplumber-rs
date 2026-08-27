//! Inspect the standard document metadata fields and validate the raw graph.
//!
//! Usage: `cargo run -p pdfplumber --example inspect_metadata -- <path-to-pdf>`

use pdfplumber::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: inspect_metadata <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(path, None)?;
    pdf.validate_metadata()?;

    let metadata = pdf.metadata();
    println!("title: {}", metadata.title.as_deref().unwrap_or("<unset>"));
    println!(
        "author: {}",
        metadata.author.as_deref().unwrap_or("<unset>")
    );
    println!(
        "subject: {}",
        metadata.subject.as_deref().unwrap_or("<unset>")
    );
    println!(
        "producer: {}",
        metadata.producer.as_deref().unwrap_or("<unset>")
    );

    Ok(())
}
