//! Report malformed input through the stable typed error category.
//!
//! Usage: `cargo run -p pdfplumber --example handle_malformed -- <path-to-pdf>`

use pdfplumber::{Pdf, PdfErrorKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: handle_malformed <path-to-pdf>",
        )
    })?;

    match Pdf::open_path(path, None) {
        Ok(pdf) => println!("Input parsed successfully with {} pages", pdf.page_count()),
        Err(error) if error.kind() == PdfErrorKind::Parse => {
            eprintln!("Malformed PDF: {error}");
        }
        Err(error) => return Err(error.into()),
    }

    Ok(())
}
