use std::env;
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};

#[derive(Debug)]
struct Request {
    implementation: String,
    workload: Option<String>,
    stage: Option<String>,
    timed: bool,
    fixture: PathBuf,
    password: Option<String>,
}

struct Execution {
    value: Value,
    wall_time_ns: Option<u128>,
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
        Ok(request)
            if request.stage.as_deref().is_some_and(|stage| {
                request.implementation == "pdf-oxide"
                    && matches!(stage, "word-grouping" | "table-detection")
            }) =>
        {
            json!({
                "status": "unsupported",
                "reason": format!(
                    "{} pinned API cannot isolate the requested word/table semantics",
                    request.implementation
                ),
            })
        }
        Ok(request) => match execute(&request) {
            Ok(execution) => {
                let mut outcome = json!({"status": "success", "value": execution.value});
                if let Some(wall_time_ns) = execution.wall_time_ns {
                    outcome["timing"] = json!({
                        "stage_id": request.stage,
                        "clock": "monotonic-wall",
                        "wall_time_ns": wall_time_ns,
                    });
                }
                outcome
            }
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
    let mut stage: Option<String> = None;
    let mut timed = false;
    let mut fixture: Option<PathBuf> = None;
    let mut password: Option<String> = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workload" => workload = arguments.next(),
            "--stage" => stage = arguments.next(),
            "--timed" => timed = true,
            "--fixture" => fixture = arguments.next().map(PathBuf::from),
            "--password" => password = arguments.next(),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if workload.is_some() == stage.is_some() {
        return Err("provide exactly one of --workload or --stage".to_string());
    }
    if timed && stage.is_none() {
        return Err("--timed requires --stage".to_string());
    }
    Ok(Request {
        implementation,
        workload,
        stage,
        timed,
        fixture: fixture.ok_or_else(|| "missing --fixture".to_string())?,
        password,
    })
}

fn execute(request: &Request) -> Result<Execution, String> {
    match request.implementation.as_str() {
        "pdfplumber-rs" => run_pdfplumber_rs(request),
        "pdf-oxide" => run_pdf_oxide(request),
        "pdfsink-rs" => run_pdfsink(request),
        _ => Err(format!(
            "unknown implementation: {}",
            request.implementation
        )),
    }
}

fn measured<T>(
    timed: bool,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<(T, Option<u128>), String> {
    let started = timed.then(Instant::now);
    let value = operation()?;
    let elapsed = started.map(|instant| instant.elapsed().as_nanos());
    Ok((value, elapsed))
}

fn run_pdfplumber_rs(request: &Request) -> Result<Execution, String> {
    if let Some(workload) = request.workload.as_deref() {
        let document = open_pdfplumber_rs(request)?;
        let value = match workload {
            "document-open" => json!({"page_count": document.page_count()}),
            "text" => pdfplumber_rs_text(&document)?,
            _ => return Err(format!("unsupported workload: {workload}")),
        };
        return Ok(Execution {
            value,
            wall_time_ns: None,
        });
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) = measured(request.timed, || open_pdfplumber_rs(request))?;
            Ok(Execution {
                value: json!({"page_count": document.page_count()}),
                wall_time_ns,
            })
        }
        "page-materialization" => {
            let document = open_pdfplumber_rs(request)?;
            let (page_numbers, wall_time_ns) = measured(request.timed, || {
                Ok((1..=document.pages().len()).collect::<Vec<_>>())
            })?;
            Ok(Execution {
                value: ordered_pages(page_numbers),
                wall_time_ns,
            })
        }
        "character-extraction" => {
            let document = open_pdfplumber_rs(request)?;
            let pages = document.pages();
            let (parsed_pages, wall_time_ns) = measured(request.timed, || {
                pages
                    .into_iter()
                    .map(|page| page.map_err(|error| error.to_string()))
                    .collect::<Result<Vec<_>, _>>()
            })?;
            Ok(Execution {
                value: Value::Array(
                    parsed_pages
                        .iter()
                        .enumerate()
                        .map(|(page_index, page)| {
                            json!({
                                "page_number": page_index + 1,
                                "chars": page
                                    .chars()
                                    .iter()
                                    .map(|character| character.text.clone())
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "word-grouping" => {
            let document = open_pdfplumber_rs(request)?;
            let pages = document
                .pages()
                .into_iter()
                .map(|page| page.map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let (words_by_page, wall_time_ns) = measured(request.timed, || {
                Ok(pages
                    .iter()
                    .map(|page| page.extract_words(&pdfplumber::WordOptions::default()))
                    .collect::<Vec<_>>())
            })?;
            Ok(Execution {
                value: Value::Array(
                    words_by_page
                        .iter()
                        .enumerate()
                        .map(|(page_index, words)| {
                            json!({
                                "page_number": page_index + 1,
                                "words": words
                                    .iter()
                                    .map(|word| json!({
                                        "text": word.text,
                                        "x0": word.bbox.x0,
                                        "top": word.bbox.top,
                                        "x1": word.bbox.x1,
                                        "bottom": word.bbox.bottom,
                                    }))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "table-detection" => {
            let document = open_pdfplumber_rs(request)?;
            let pages = document
                .pages()
                .into_iter()
                .map(|page| page.map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let (tables_by_page, wall_time_ns) = measured(request.timed, || {
                Ok(pages
                    .iter()
                    .map(|page| page.extract_tables(&pdfplumber::TableSettings::default()))
                    .collect::<Vec<_>>())
            })?;
            Ok(Execution {
                value: Value::Array(
                    tables_by_page
                        .into_iter()
                        .enumerate()
                        .map(|(page_index, tables)| {
                            json!({"page_number": page_index + 1, "tables": tables})
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "serialization" => {
            let document = open_pdfplumber_rs(request)?;
            let canonical = pdfplumber_rs_text(&document)?;
            serialize_canonical(canonical, request.timed)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn open_pdfplumber_rs(request: &Request) -> Result<pdfplumber::Pdf, String> {
    match request.password.as_deref() {
        Some(password) => {
            pdfplumber::Pdf::open_path_with_password(&request.fixture, password.as_bytes(), None)
        }
        None => pdfplumber::Pdf::open_path(&request.fixture, None),
    }
    .map_err(|error| error.to_string())
}

fn pdfplumber_rs_text(document: &pdfplumber::Pdf) -> Result<Value, String> {
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

fn run_pdf_oxide(request: &Request) -> Result<Execution, String> {
    if let Some(workload) = request.workload.as_deref() {
        let document = open_pdf_oxide(request)?;
        let value = match workload {
            "document-open" => json!({
                "page_count": document.page_count().map_err(|error| error.to_string())?
            }),
            "text" => pdf_oxide_text(&document)?,
            _ => return Err(format!("unsupported workload: {workload}")),
        };
        return Ok(Execution {
            value,
            wall_time_ns: None,
        });
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) = measured(request.timed, || open_pdf_oxide(request))?;
            Ok(Execution {
                value: json!({
                    "page_count": document.page_count().map_err(|error| error.to_string())?
                }),
                wall_time_ns,
            })
        }
        "page-materialization" => {
            let document = open_pdf_oxide(request)?;
            let page_count = document.page_count().map_err(|error| error.to_string())?;
            let (pages, wall_time_ns) = measured(request.timed, || {
                (0..page_count)
                    .map(|page_index| {
                        document
                            .get_page(page_index)
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()
            })?;
            Ok(Execution {
                value: ordered_pages((1..=pages.len()).collect()),
                wall_time_ns,
            })
        }
        "character-extraction" => {
            let document = open_pdf_oxide(request)?;
            let page_count = document.page_count().map_err(|error| error.to_string())?;
            for page_index in 0..page_count {
                document
                    .get_page(page_index)
                    .map_err(|error| error.to_string())?;
            }
            let (chars_by_page, wall_time_ns) = measured(request.timed, || {
                (0..page_count)
                    .map(|page_index| {
                        document
                            .extract_chars(page_index)
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()
            })?;
            Ok(Execution {
                value: Value::Array(
                    chars_by_page
                        .iter()
                        .enumerate()
                        .map(|(page_index, characters)| {
                            json!({
                                "page_number": page_index + 1,
                                "chars": characters
                                    .iter()
                                    .map(|character| character.char.to_string())
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "serialization" => {
            let document = open_pdf_oxide(request)?;
            let canonical = pdf_oxide_text(&document)?;
            serialize_canonical(canonical, request.timed)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn open_pdf_oxide(request: &Request) -> Result<pdf_oxide::PdfDocument, String> {
    pdf_oxide::PdfDocument::open(&request.fixture).map_err(|error| error.to_string())
}

fn pdf_oxide_text(document: &pdf_oxide::PdfDocument) -> Result<Value, String> {
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

fn run_pdfsink(request: &Request) -> Result<Execution, String> {
    if let Some(workload) = request.workload.as_deref() {
        let document = open_pdfsink(request)?;
        let value = match workload {
            "document-open" => json!({"page_count": document.len()}),
            "text" => pdfsink_text(&document),
            _ => return Err(format!("unsupported workload: {workload}")),
        };
        return Ok(Execution {
            value,
            wall_time_ns: None,
        });
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) = measured(request.timed, || open_pdfsink(request))?;
            Ok(Execution {
                value: json!({"page_count": document.len()}),
                wall_time_ns,
            })
        }
        "page-materialization" => {
            let document = open_pdfsink(request)?;
            Ok(Execution {
                value: ordered_pages((1..=document.pages().len()).collect()),
                wall_time_ns: None,
            })
        }
        "character-extraction" => {
            let document = open_pdfsink(request)?;
            Ok(Execution {
                value: Value::Array(
                    document
                        .pages()
                        .iter()
                        .enumerate()
                        .map(|(page_index, page)| {
                            json!({
                                "page_number": page_index + 1,
                                "chars": page
                                    .chars
                                    .iter()
                                    .map(|character| character.text.clone())
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
                wall_time_ns: None,
            })
        }
        "word-grouping" => {
            let document = open_pdfsink(request)?;
            let (words_by_page, wall_time_ns) = measured(request.timed, || {
                Ok(document
                    .pages()
                    .iter()
                    .map(|page| {
                        page.extract_words_with_options(&pdfsink_rs::TextOptions::default(), false)
                    })
                    .collect::<Vec<_>>())
            })?;
            Ok(Execution {
                value: Value::Array(
                    words_by_page
                        .iter()
                        .enumerate()
                        .map(|(page_index, words)| {
                            json!({
                                "page_number": page_index + 1,
                                "words": words
                                    .iter()
                                    .map(|word| json!({
                                        "text": word.text,
                                        "x0": word.x0,
                                        "top": word.top,
                                        "x1": word.x1,
                                        "bottom": word.bottom,
                                    }))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "table-detection" => {
            let document = open_pdfsink(request)?;
            let (tables_by_page, wall_time_ns) = measured(request.timed, || {
                document
                    .pages()
                    .iter()
                    .map(|page| {
                        page.extract_tables(pdfsink_rs::TableSettings::default())
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()
            })?;
            Ok(Execution {
                value: Value::Array(
                    tables_by_page
                        .into_iter()
                        .enumerate()
                        .map(|(page_index, tables)| {
                            json!({"page_number": page_index + 1, "tables": tables})
                        })
                        .collect(),
                ),
                wall_time_ns,
            })
        }
        "serialization" => {
            let document = open_pdfsink(request)?;
            let canonical = pdfsink_text(&document);
            serialize_canonical(canonical, request.timed)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn open_pdfsink(request: &Request) -> Result<pdfsink_rs::PdfDocument, String> {
    pdfsink_rs::PdfDocument::open(&request.fixture).map_err(|error| error.to_string())
}

fn pdfsink_text(document: &pdfsink_rs::PdfDocument) -> Value {
    Value::Array(
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
    )
}

fn ordered_pages(page_numbers: Vec<usize>) -> Value {
    Value::Array(
        page_numbers
            .into_iter()
            .map(|page_number| json!({"page_number": page_number}))
            .collect(),
    )
}

fn serialize_canonical(canonical: Value, timed: bool) -> Result<Execution, String> {
    let (serialized, wall_time_ns) = measured(timed, || {
        serde_json::to_string(&canonical).map_err(|error| error.to_string())
    })?;
    Ok(Execution {
        value: json!({
            "utf8": serialized,
            "utf8_bytes": serialized.len(),
        }),
        wall_time_ns,
    })
}
