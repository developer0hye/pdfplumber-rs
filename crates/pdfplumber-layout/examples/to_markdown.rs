//! Convert a PDF to GitHub-Flavored Markdown via layout inference.
//!
//! Produces heading levels (`#` `##` `###`), paragraph text, pipe-delimited
//! tables, and figure placeholders. Ideal for feeding PDF content to LLMs,
//! generating documentation from PDFs, or search indexing.
//!
//! Run with:
//!   `cargo run --example to_markdown -p pdfplumber-layout -- <file.pdf>`
//!   `cargo run --example to_markdown -p pdfplumber-layout -- <file.pdf> out.md`

use std::env;
use std::fs;
use std::process::ExitCode;

use pdfplumber::Pdf;
use pdfplumber_layout::Document;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: to_markdown <file.pdf> [output.md]")?;
    let out = args.next();

    let pdf = Pdf::open_file(&path, None)?;
    let doc = Document::from_pdf(&pdf);
    let md = doc.to_markdown();

    match out.as_deref() {
        Some(p) => {
            fs::write(p, &md)?;
            println!("wrote {} chars → {p}", md.len());
        }
        None => print!("{md}"),
    }

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
