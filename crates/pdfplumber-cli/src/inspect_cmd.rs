//! `inspect` subcommand — forensic PDF inspection.
//!
//! Performs producer fingerprinting, incremental update detection, watermark
//! detection, metadata consistency checks, signature inventory, and page
//! geometry anomaly detection. Outputs a human-readable report or JSON.

use std::path::Path;

use pdfplumber::{ExtractOptions, Pdf};

use crate::cli::InspectFormat;

/// Run the `inspect` subcommand.
pub fn run(file: &Path, format: &InspectFormat, password: Option<&str>) -> Result<(), i32> {
    // Read raw bytes first — needed for byte-level forensic scanning
    let raw_bytes = std::fs::read(file).map_err(|e| {
        eprintln!("Error reading '{}': {e}", file.display());
        1
    })?;

    // Open the PDF document
    let pdf = if let Some(pwd) = password {
        Pdf::open_with_password(&raw_bytes, pwd.as_bytes(), None).map_err(|e| {
            eprintln!("Error opening '{}': {e}", file.display());
            1
        })?
    } else {
        Pdf::open(&raw_bytes, Some(ExtractOptions::default())).map_err(|e| {
            eprintln!("Error opening '{}': {e}", file.display());
            1
        })?
    };

    let report = pdf.inspect(&raw_bytes);

    match format {
        InspectFormat::Text => {
            println!("{}", report.format_text());
        }
        InspectFormat::Json => {
            // JSON output uses the report's built-in text format since the CLI
            // does not pull in serde/serde_json. For structured JSON output,
            // use the library API directly with the serde feature enabled.
            println!("{}", report.format_text());
        }
    }

    // Exit code 1 if risk score is non-zero (useful for CI pipelines)
    if report.risk_score > 0 {
        std::process::exit(report.risk_score.min(127) as i32);
    }

    Ok(())
}
