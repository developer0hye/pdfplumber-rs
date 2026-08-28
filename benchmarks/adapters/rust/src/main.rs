use std::env;
use std::path::PathBuf;

use serde_json::{Value, json};

#[derive(Debug)]
struct Request {
    implementation: String,
    workload: String,
    fixture: PathBuf,
    password: Option<String>,
}

fn main() {
    let outcome = match parse_request() {
        Ok(request)
            if request.password.is_some()
                && matches!(request.implementation.as_str(), "pdf-oxide" | "pdfsink-rs") =>
        {
            json!({
                "status": "unsupported",
                "reason": format!(
                    "{} pinned API does not expose the fixture password option",
                    request.implementation
                ),
            })
        }
        Ok(request) => match execute(request) {
            Ok(value) => json!({"status": "success", "value": value}),
            Err(message) => json!({
                "status": "error",
                "error": {"kind": "adapter", "message": message},
            }),
        },
        Err(message) => json!({
            "status": "error",
            "error": {"kind": "adapter", "message": message},
        }),
    };
    println!(
        "{}",
        serde_json::to_string(&outcome).expect("benchmark outcome is JSON")
    );
}

fn parse_request() -> Result<Request, String> {
    let mut arguments = env::args().skip(1);
    let implementation = arguments
        .next()
        .ok_or_else(|| "missing implementation".to_string())?;
    let mut workload: Option<String> = None;
    let mut fixture: Option<PathBuf> = None;
    let mut password: Option<String> = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workload" => workload = arguments.next(),
            "--fixture" => fixture = arguments.next().map(PathBuf::from),
            "--password" => password = arguments.next(),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(Request {
        implementation,
        workload: workload.ok_or_else(|| "missing --workload".to_string())?,
        fixture: fixture.ok_or_else(|| "missing --fixture".to_string())?,
        password,
    })
}

fn execute(request: Request) -> Result<Value, String> {
    match request.implementation.as_str() {
        "pdfplumber-rs" => run_pdfplumber_rs(&request),
        "pdf-oxide" => run_pdf_oxide(&request),
        "pdfsink-rs" => run_pdfsink(&request),
        _ => Err(format!(
            "unknown implementation: {}",
            request.implementation
        )),
    }
}

fn run_pdfplumber_rs(request: &Request) -> Result<Value, String> {
    let document = match request.password.as_deref() {
        Some(password) => {
            pdfplumber::Pdf::open_path_with_password(&request.fixture, password.as_bytes(), None)
        }
        None => pdfplumber::Pdf::open_path(&request.fixture, None),
    }
    .map_err(|error| error.to_string())?;
    match request.workload.as_str() {
        "document-open" => Ok(json!({"page_count": document.page_count()})),
        "text" => {
            let mut pages = Vec::with_capacity(document.page_count());
            for (page_index, page) in document.pages().into_iter().enumerate() {
                let page = page.map_err(|error| error.to_string())?;
                pages.push(json!({
                    "page_number": page_index + 1,
                    "text": page.extract_text(&pdfplumber::TextOptions::default()),
                }));
            }
            Ok(Value::Array(pages))
        }
        workload => Err(format!("unsupported workload: {workload}")),
    }
}

fn run_pdf_oxide(request: &Request) -> Result<Value, String> {
    let document =
        pdf_oxide::PdfDocument::open(&request.fixture).map_err(|error| error.to_string())?;
    match request.workload.as_str() {
        "document-open" => Ok(json!({
            "page_count": document.page_count().map_err(|error| error.to_string())?
        })),
        "text" => {
            let page_count = document.page_count().map_err(|error| error.to_string())?;
            let mut pages = Vec::with_capacity(page_count);
            for page_index in 0..page_count {
                pages.push(json!({
                    "page_number": page_index + 1,
                    "text": document
                        .extract_text(page_index)
                        .map_err(|error| error.to_string())?,
                }));
            }
            Ok(Value::Array(pages))
        }
        workload => Err(format!("unsupported workload: {workload}")),
    }
}

fn run_pdfsink(request: &Request) -> Result<Value, String> {
    let document =
        pdfsink_rs::PdfDocument::open(&request.fixture).map_err(|error| error.to_string())?;
    match request.workload.as_str() {
        "document-open" => Ok(json!({"page_count": document.len()})),
        "text" => Ok(Value::Array(
            document
                .pages()
                .iter()
                .enumerate()
                .map(|(page_index, page)| {
                    json!({
                        "page_number": page_index + 1,
                        "text": page.extract_text(),
                    })
                })
                .collect(),
        )),
        workload => Err(format!("unsupported workload: {workload}")),
    }
}
