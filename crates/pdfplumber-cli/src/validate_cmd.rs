use std::path::Path;

use pdfplumber::Severity;

use crate::cli::ValidateFormat;
use crate::shared::open_pdf_full;

pub fn run(file: &Path, format: &ValidateFormat, password: Option<&str>) -> Result<(), i32> {
    let pdf = open_pdf_full(file, None, password)?;

    let issues = pdf.validate().map_err(|e| {
        eprintln!("Error: validation failed: {e}");
        1
    })?;

    let error_count = issues.iter().filter(|i| i.is_error()).count();
    let warning_count = issues.iter().filter(|i| i.is_warning()).count();

    match format {
        ValidateFormat::Text => {
            if issues.is_empty() {
                println!("No issues found.");
            } else {
                for issue in &issues {
                    let severity = match issue.severity {
                        Severity::Error => "ERROR",
                        Severity::Warning => "WARNING",
                    };
                    print!("[{severity}] {}: {}", issue.code, issue.message);
                    if let Some(ref loc) = issue.location {
                        print!(" (at {loc})");
                    }
                    println!();
                }
                println!();
                println!("Summary: {error_count} error(s), {warning_count} warning(s)");
            }
        }
        ValidateFormat::Json => {
            let issues_json: Vec<serde_json::Value> = issues
                .iter()
                .map(|issue| {
                    let mut obj = serde_json::json!({
                        "severity": issue.severity.to_string(),
                        "code": issue.code,
                        "message": issue.message,
                    });
                    if let Some(ref loc) = issue.location {
                        obj["location"] = serde_json::json!(loc);
                    }
                    obj
                })
                .collect();

            let output = serde_json::json!({
                "issues": issues_json,
                "summary": {
                    "errors": error_count,
                    "warnings": warning_count,
                },
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    Ok(())
}
