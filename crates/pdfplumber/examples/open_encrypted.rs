//! Open a password-protected PDF without printing its password.
//!
//! Usage: `cargo run -p pdfplumber --example open_encrypted -- <path-to-pdf> <password>`

use pdfplumber::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: open_encrypted <path-to-pdf> <password>",
        )
    })?;
    let password = arguments.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: open_encrypted <path-to-pdf> <password>",
        )
    })?;

    let pdf = Pdf::open_path_with_password(path, password.as_bytes(), None)?;
    println!("Opened encrypted document with {} pages", pdf.page_count());

    Ok(())
}
