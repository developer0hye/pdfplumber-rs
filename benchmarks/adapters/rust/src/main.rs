use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use serde_json::{Value, json};

struct CountingAllocator;

static ALLOCATION_TRACKING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to the process System allocator with the
// original layout. The atomics only observe successful allocation requests.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the unchanged layout satisfies System::alloc.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && ALLOCATION_TRACKING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the unchanged layout satisfies System::alloc_zeroed.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && ALLOCATION_TRACKING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout came from the delegated System allocator.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarding the allocation and original layout satisfies System::realloc.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() && ALLOCATION_TRACKING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        new_pointer
    }
}

#[derive(Debug)]
struct ResourceMetrics {
    cpu_time_ns: u128,
    peak_resident_memory_bytes: u64,
    gross_allocation_count: u64,
    gross_allocated_bytes: u64,
}

static LAST_RESOURCES: Mutex<Option<ResourceMetrics>> = Mutex::new(None);

#[derive(Debug)]
struct Request {
    implementation: String,
    workload: Option<String>,
    stage: Option<String>,
    scenario: Option<String>,
    timed: bool,
    resources: bool,
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
                    outcome["timing"] = if let Some(scenario_id) = &request.scenario {
                        json!({
                            "scenario_id": scenario_id,
                            "clock": "monotonic-wall",
                            "wall_time_ns": wall_time_ns,
                        })
                    } else {
                        json!({
                            "stage_id": request.stage,
                            "clock": "monotonic-wall",
                            "wall_time_ns": wall_time_ns,
                        })
                    };
                }
                if request.resources {
                    match take_resource_metrics() {
                        Ok(resources) => {
                            outcome["resources"] = json!({
                                "stage_id": request.stage,
                                "cpu": {
                                    "clock": "process-cpu",
                                    "scope": "in-adapter-stage-only",
                                    "time_ns": resources.cpu_time_ns,
                                },
                                "peak_resident_memory": {
                                    "scope": "adapter-process-lifetime-high-water",
                                    "bytes": resources.peak_resident_memory_bytes,
                                },
                                "allocations": {
                                    "method": "rust-counting-global-allocator",
                                    "scope": "in-adapter-stage-only",
                                    "gross_allocation_count": resources.gross_allocation_count,
                                    "gross_allocated_bytes": resources.gross_allocated_bytes,
                                },
                            });
                        }
                        Err(message) => {
                            outcome = json!({
                                "status": "error",
                                "error": {"kind": "adapter", "message": message},
                            });
                        }
                    }
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
    let mut scenario: Option<String> = None;
    let mut timed = false;
    let mut resources = false;
    let mut fixture: Option<PathBuf> = None;
    let mut password: Option<String> = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workload" => workload = arguments.next(),
            "--stage" => stage = arguments.next(),
            "--scenario" => scenario = arguments.next(),
            "--timed" => timed = true,
            "--resources" => resources = true,
            "--fixture" => fixture = arguments.next().map(PathBuf::from),
            "--password" => password = arguments.next(),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if usize::from(workload.is_some())
        + usize::from(stage.is_some())
        + usize::from(scenario.is_some())
        != 1
    {
        return Err("provide exactly one of --workload, --stage, or --scenario".to_string());
    }
    if timed && stage.is_none() && scenario.is_none() {
        return Err("--timed requires --stage or --scenario".to_string());
    }
    if resources && stage.is_none() {
        return Err("--resources requires --stage".to_string());
    }
    if timed && resources {
        return Err("--timed and --resources are separate passes".to_string());
    }
    Ok(Request {
        implementation,
        workload,
        stage,
        scenario,
        timed,
        resources,
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
    resources: bool,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<(T, Option<u128>), String> {
    if resources {
        let cpu_started_ns = process_cpu_time_ns()?;
        start_resource_metrics()?;
        let result = operation();
        let allocations = stop_allocation_metrics();
        let cpu_finished_ns = process_cpu_time_ns();
        let value = result?;
        let cpu_time_ns = cpu_finished_ns?.saturating_sub(cpu_started_ns);
        let peak_resident_memory_bytes = peak_resident_memory_bytes()?;
        store_resource_metrics(ResourceMetrics {
            cpu_time_ns,
            peak_resident_memory_bytes,
            gross_allocation_count: allocations.0,
            gross_allocated_bytes: allocations.1,
        })?;
        return Ok((value, None));
    }
    let started = timed.then(Instant::now);
    let value = operation()?;
    let elapsed = started.map(|instant| instant.elapsed().as_nanos());
    Ok((value, elapsed))
}

fn start_resource_metrics() -> Result<(), String> {
    let mut last = LAST_RESOURCES
        .lock()
        .map_err(|_| "resource metric lock is poisoned".to_string())?;
    *last = None;
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    ALLOCATION_TRACKING.store(true, Ordering::SeqCst);
    Ok(())
}

fn stop_allocation_metrics() -> (u64, u64) {
    ALLOCATION_TRACKING.store(false, Ordering::SeqCst);
    (
        ALLOCATION_COUNT.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
    )
}

fn store_resource_metrics(resources: ResourceMetrics) -> Result<(), String> {
    let mut last = LAST_RESOURCES
        .lock()
        .map_err(|_| "resource metric lock is poisoned".to_string())?;
    *last = Some(resources);
    Ok(())
}

fn take_resource_metrics() -> Result<ResourceMetrics, String> {
    LAST_RESOURCES
        .lock()
        .map_err(|_| "resource metric lock is poisoned".to_string())?
        .take()
        .ok_or_else(|| "resource pass did not observe the requested stage".to_string())
}

fn process_cpu_time_ns() -> Result<u128, String> {
    let usage = process_usage()?;
    let user_ns = timeval_ns(usage.ru_utime)?;
    let system_ns = timeval_ns(usage.ru_stime)?;
    Ok(user_ns + system_ns)
}

fn peak_resident_memory_bytes() -> Result<u64, String> {
    let maximum = process_usage()?.ru_maxrss;
    let maximum =
        u64::try_from(maximum).map_err(|_| "peak resident memory is negative".to_string())?;
    #[cfg(target_os = "macos")]
    return Ok(maximum);
    #[cfg(not(target_os = "macos"))]
    return maximum
        .checked_mul(1024)
        .ok_or_else(|| "peak resident memory overflowed bytes".to_string());
}

fn process_usage() -> Result<libc::rusage, String> {
    // SAFETY: zero is a valid initial byte representation for rusage, and
    // getrusage initializes the structure before it is read on success.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: usage is a valid writable pointer for the duration of the call.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    if result == 0 {
        Ok(usage)
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn timeval_ns(value: libc::timeval) -> Result<u128, String> {
    let seconds =
        u128::try_from(value.tv_sec).map_err(|_| "process CPU seconds are negative".to_string())?;
    let microseconds = u128::try_from(value.tv_usec)
        .map_err(|_| "process CPU microseconds are negative".to_string())?;
    Ok(seconds * 1_000_000_000 + microseconds * 1_000)
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

    if let Some(scenario) = request.scenario.as_deref() {
        return run_pdfplumber_rs_scenario(request, scenario);
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) = measured(request.timed, request.resources, || {
                open_pdfplumber_rs(request)
            })?;
            Ok(Execution {
                value: json!({"page_count": document.page_count()}),
                wall_time_ns,
            })
        }
        "page-materialization" => {
            let document = open_pdfplumber_rs(request)?;
            let (page_numbers, wall_time_ns) = measured(request.timed, request.resources, || {
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
            let (parsed_pages, wall_time_ns) = measured(request.timed, request.resources, || {
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
            let (words_by_page, wall_time_ns) = measured(request.timed, request.resources, || {
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
            let (tables_by_page, wall_time_ns) =
                measured(request.timed, request.resources, || {
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
            serialize_canonical(canonical, request.timed, request.resources)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn run_pdfplumber_rs_scenario(
    request: &Request,
    scenario: &str,
) -> Result<Execution, String> {
    match scenario {
        "cold-document-open" => {
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdfplumber_rs(request))?;
            Ok(Execution {
                value: json!({"page_count": document.page_count()}),
                wall_time_ns,
            })
        }
        "warm-document-open" => {
            let warm_document = open_pdfplumber_rs(request)?;
            let _ = warm_document.page_count();
            drop(warm_document);
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdfplumber_rs(request))?;
            Ok(Execution {
                value: json!({"page_count": document.page_count()}),
                wall_time_ns,
            })
        }
        "single-page-text" => {
            let document = open_pdfplumber_rs(request)?;
            let (value, wall_time_ns) = measured(request.timed, false, || {
                let page = document.page(0).map_err(|error| error.to_string())?;
                Ok(Value::Array(vec![json!({
                    "page_number": 1,
                    "text": page.extract_text(&pdfplumber::TextOptions::default()),
                })]))
            })?;
            Ok(Execution {
                value,
                wall_time_ns,
            })
        }
        "full-document-text" => {
            let document = open_pdfplumber_rs(request)?;
            let (value, wall_time_ns) = measured(request.timed, false, || {
                pdfplumber_rs_text(&document)
            })?;
            Ok(Execution {
                value,
                wall_time_ns,
            })
        }
        "parallel-page-batch-text" => {
            let document = open_pdfplumber_rs(request)?;
            let thread_pool = ThreadPoolBuilder::new()
                .num_threads(4)
                .build()
                .map_err(|error| error.to_string())?;
            let (value, wall_time_ns) = measured(request.timed, false, || {
                thread_pool.install(|| {
                    let pages = document
                        .pages_parallel()
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| error.to_string())?;
                    let ordered = pages
                        .into_par_iter()
                        .enumerate()
                        .map(|(page_index, page)| {
                            json!({
                                "page_number": page_index + 1,
                                "text": page.extract_text(&pdfplumber::TextOptions::default()),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok(Value::Array(ordered))
                })
            })?;
            Ok(Execution {
                value,
                wall_time_ns,
            })
        }
        _ => Err(format!("unsupported scenario: {scenario}")),
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

    if let Some(scenario) = request.scenario.as_deref() {
        return run_pdf_oxide_scenario(request, scenario);
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) =
                measured(request.timed, request.resources, || open_pdf_oxide(request))?;
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
            let (pages, wall_time_ns) = measured(request.timed, request.resources, || {
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
            let (chars_by_page, wall_time_ns) = measured(request.timed, request.resources, || {
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
            serialize_canonical(canonical, request.timed, request.resources)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn run_pdf_oxide_scenario(request: &Request, scenario: &str) -> Result<Execution, String> {
    match scenario {
        "cold-document-open" => {
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdf_oxide(request))?;
            Ok(Execution {
                value: json!({
                    "page_count": document.page_count().map_err(|error| error.to_string())?
                }),
                wall_time_ns,
            })
        }
        "warm-document-open" => {
            let warm_document = open_pdf_oxide(request)?;
            let _ = warm_document
                .page_count()
                .map_err(|error| error.to_string())?;
            drop(warm_document);
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdf_oxide(request))?;
            Ok(Execution {
                value: json!({
                    "page_count": document.page_count().map_err(|error| error.to_string())?
                }),
                wall_time_ns,
            })
        }
        "single-page-text" => {
            let document = open_pdf_oxide(request)?;
            let (text, wall_time_ns) = measured(request.timed, false, || {
                document.extract_text(0).map_err(|error| error.to_string())
            })?;
            Ok(Execution {
                value: Value::Array(vec![json!({"page_number": 1, "text": text})]),
                wall_time_ns,
            })
        }
        "full-document-text" => {
            let document = open_pdf_oxide(request)?;
            let (value, wall_time_ns) =
                measured(request.timed, false, || pdf_oxide_text(&document))?;
            Ok(Execution {
                value,
                wall_time_ns,
            })
        }
        _ => Err(format!("unsupported scenario: {scenario}")),
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

    if let Some(scenario) = request.scenario.as_deref() {
        return run_pdfsink_scenario(request, scenario);
    }

    let stage = request.stage.as_deref().expect("stage was validated");
    match stage {
        "document-open" => {
            let (document, wall_time_ns) =
                measured(request.timed, request.resources, || open_pdfsink(request))?;
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
            let (words_by_page, wall_time_ns) = measured(request.timed, request.resources, || {
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
            let (tables_by_page, wall_time_ns) =
                measured(request.timed, request.resources, || {
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
            serialize_canonical(canonical, request.timed, request.resources)
        }
        _ => Err(format!("unsupported stage: {stage}")),
    }
}

fn run_pdfsink_scenario(request: &Request, scenario: &str) -> Result<Execution, String> {
    match scenario {
        "cold-document-open" => {
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdfsink(request))?;
            Ok(Execution {
                value: json!({"page_count": document.len()}),
                wall_time_ns,
            })
        }
        "warm-document-open" => {
            let warm_document = open_pdfsink(request)?;
            let _ = warm_document.len();
            drop(warm_document);
            let (document, wall_time_ns) =
                measured(request.timed, false, || open_pdfsink(request))?;
            Ok(Execution {
                value: json!({"page_count": document.len()}),
                wall_time_ns,
            })
        }
        "single-page-text" => {
            let document = open_pdfsink(request)?;
            let (text, wall_time_ns) = measured(request.timed, false, || {
                document
                    .pages()
                    .first()
                    .map(|page| page.extract_text())
                    .ok_or_else(|| "document has no pages".to_string())
            })?;
            Ok(Execution {
                value: Value::Array(vec![json!({"page_number": 1, "text": text})]),
                wall_time_ns,
            })
        }
        "full-document-text" => {
            let document = open_pdfsink(request)?;
            let (value, wall_time_ns) =
                measured(request.timed, false, || Ok(pdfsink_text(&document)))?;
            Ok(Execution {
                value,
                wall_time_ns,
            })
        }
        _ => Err(format!("unsupported scenario: {scenario}")),
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

fn serialize_canonical(
    canonical: Value,
    timed: bool,
    resources: bool,
) -> Result<Execution, String> {
    let (serialized, wall_time_ns) = measured(timed, resources, || {
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
