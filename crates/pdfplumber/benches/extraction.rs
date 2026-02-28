//! Performance benchmarks for pdfplumber-rs.
//!
//! Benchmarks cover the full extraction pipeline: char extraction, word extraction,
//! text extraction, and table detection (lattice + stream) across three PDF sizes:
//! - Simple: 1-page, single paragraph
//! - Medium: 10-page, multiple paragraphs per page
//! - Complex: 10-page, tables + multi-font + dense text

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use lopdf::{Object, Stream, dictionary};
use pdfplumber::{Pdf, Strategy, TableSettings, TextOptions, WordOptions};

// ---------------------------------------------------------------------------
// PDF fixture generators
// ---------------------------------------------------------------------------

/// Build a single-page PDF containing the given content streams.
/// Each content string becomes a separate page.
fn build_pdf(contents: &[Vec<u8>]) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.5");

    // Shared font resources
    let font_f1 = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let font_f2 = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    let mut page_ids = Vec::new();
    for content in contents {
        let stream = Stream::new(dictionary! {}, content.clone());
        let content_id = doc.add_object(stream);

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_f1),
                "F2" => Object::Reference(font_f2),
            },
        };

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box.clone(),
            "Contents" => Object::Reference(content_id),
            "Resources" => resources,
        };
        page_ids.push(doc.add_object(page_dict));
    }

    let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => Object::Integer(contents.len() as i64),
    };
    let pages_id = doc.add_object(pages_dict);

    for &pid in &page_ids {
        if let Ok(obj) = doc.get_object_mut(pid) {
            if let Ok(dict) = obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Escape a string for a PDF literal string: `(` → `\(`, `)` → `\)`, `\` → `\\`.
fn pdf_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Generate a content stream with `n_lines` lines of text, each ~60 chars.
fn text_content(n_lines: usize) -> Vec<u8> {
    let mut ops = String::from("BT\n/F1 10 Tf\n72 720 Td\n");
    for i in 0..n_lines {
        let line = format!(
            "Line {} of the document with some words to measure extraction speed here",
            i + 1
        );
        if i > 0 {
            ops.push_str("0 -14 Td\n");
        }
        ops.push_str(&format!("({}) Tj\n", pdf_escape(&line)));
    }
    ops.push_str("ET\n");
    ops.into_bytes()
}

/// Generate a content stream with a table drawn via line operators (lattice).
/// Creates an `rows x cols` grid starting at (72, 150) with cells 80pt wide, 20pt tall.
fn table_content_lattice(rows: usize, cols: usize) -> Vec<u8> {
    let x_start = 72.0_f64;
    let y_start = 150.0;
    let cell_w = 80.0;
    let cell_h = 20.0;
    let table_w = cols as f64 * cell_w;
    let table_h = rows as f64 * cell_h;

    let mut ops = String::new();

    // Draw horizontal lines
    for r in 0..=rows {
        let y = y_start + r as f64 * cell_h;
        ops.push_str(&format!("{x_start} {y} m {} {y} l S\n", x_start + table_w));
    }
    // Draw vertical lines
    for c in 0..=cols {
        let x = x_start + c as f64 * cell_w;
        ops.push_str(&format!("{x} {y_start} m {x} {} l S\n", y_start + table_h));
    }

    // Put text in each cell (centre-ish of cell)
    ops.push_str("BT\n/F1 8 Tf\n");
    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f64 * cell_w + 4.0;
            // PDF y-coord (bottom-up), place text near top of cell
            let y = y_start + r as f64 * cell_h + 6.0;
            let text = format!("R{}C{}", r + 1, c + 1);
            ops.push_str(&format!("{x} {y} Td ({text}) Tj\n"));
        }
    }
    ops.push_str("ET\n");

    ops.into_bytes()
}

/// Generate a content stream with text arranged in a grid pattern (no visible lines)
/// for stream-strategy table detection.
fn table_content_stream(rows: usize, cols: usize) -> Vec<u8> {
    let x_start = 72.0_f64;
    let y_start = 720.0; // Start near top in PDF coords
    let col_width = 100.0;
    let row_height = 16.0;

    let mut ops = String::from("BT\n/F1 10 Tf\n");
    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f64 * col_width;
            let y = y_start - r as f64 * row_height;
            let text = format!("Data-{}-{}", r + 1, c + 1);
            ops.push_str(&format!("{x} {y} Td ({text}) Tj\n"));
        }
    }
    ops.push_str("ET\n");
    ops.into_bytes()
}

/// Generate a complex page: mixed fonts, dense text, and a lattice table.
fn complex_page_content(page_idx: usize) -> Vec<u8> {
    let mut ops = String::new();

    // Header in Courier
    ops.push_str("BT\n/F2 14 Tf\n72 750 Td\n");
    let header = format!("Document Section {} - Complex Layout", page_idx + 1);
    ops.push_str(&format!("({}) Tj\n", pdf_escape(&header)));
    ops.push_str("ET\n");

    // Body text in Helvetica — 15 lines
    ops.push_str("BT\n/F1 10 Tf\n72 720 Td\n");
    for i in 0..15 {
        if i > 0 {
            ops.push_str("0 -14 Td\n");
        }
        let line = format!(
            "Paragraph {} text with mixed content, numbers 12345 and punctuation marks!",
            i + 1
        );
        ops.push_str(&format!("({}) Tj\n", pdf_escape(&line)));
    }
    ops.push_str("ET\n");

    // Embedded 5x4 lattice table in the lower half
    let x_start = 72.0_f64;
    let y_start = 350.0; // PDF coords
    let cell_w = 90.0;
    let cell_h = 18.0;
    let table_rows = 5_usize;
    let table_cols = 4_usize;
    let table_w = table_cols as f64 * cell_w;
    let table_h = table_rows as f64 * cell_h;

    for r in 0..=table_rows {
        let y = y_start + r as f64 * cell_h;
        ops.push_str(&format!("{x_start} {y} m {} {y} l S\n", x_start + table_w));
    }
    for c in 0..=table_cols {
        let x = x_start + c as f64 * cell_w;
        ops.push_str(&format!("{x} {y_start} m {x} {} l S\n", y_start + table_h));
    }

    // Table cell text
    ops.push_str("BT\n/F1 8 Tf\n");
    for r in 0..table_rows {
        for c in 0..table_cols {
            let x = x_start + c as f64 * cell_w + 4.0;
            let y = y_start + r as f64 * cell_h + 5.0;
            let text = format!("Cell{}{}", r + 1, c + 1);
            ops.push_str(&format!("{x} {y} Td ({text}) Tj\n"));
        }
    }
    ops.push_str("ET\n");

    ops.into_bytes()
}

// ---------------------------------------------------------------------------
// Fixture caching (built once, reused across iterations)
// ---------------------------------------------------------------------------

/// Simple PDF: 1 page, 10 lines of text.
fn simple_pdf_bytes() -> Vec<u8> {
    build_pdf(&[text_content(10)])
}

/// Medium PDF: 10 pages, 30 lines each.
fn medium_pdf_bytes() -> Vec<u8> {
    let pages: Vec<Vec<u8>> = (0..10).map(|_| text_content(30)).collect();
    build_pdf(&pages)
}

/// Complex PDF: 10 pages, each with header + body text + embedded table.
fn complex_pdf_bytes() -> Vec<u8> {
    let pages: Vec<Vec<u8>> = (0..10).map(complex_page_content).collect();
    build_pdf(&pages)
}

/// PDF with a lattice table: 1 page, 20x5 grid.
fn lattice_table_pdf_bytes() -> Vec<u8> {
    build_pdf(&[table_content_lattice(20, 5)])
}

/// PDF with text-only table (stream strategy): 1 page, 20x5 grid.
fn stream_table_pdf_bytes() -> Vec<u8> {
    build_pdf(&[table_content_stream(20, 5)])
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_char_extraction(c: &mut Criterion) {
    let simple = simple_pdf_bytes();
    let medium = medium_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("char_extraction");

    group.bench_function("simple_1page", |b| {
        let pdf = Pdf::open(&simple, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.chars().len());
        });
    });

    group.bench_function("medium_10page", |b| {
        let pdf = Pdf::open(&medium, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.chars().len());
            }
        });
    });

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.chars().len());
            }
        });
    });

    group.finish();
}

fn bench_word_extraction(c: &mut Criterion) {
    let simple = simple_pdf_bytes();
    let medium = medium_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("word_extraction");
    let opts = WordOptions::default();

    group.bench_function("simple_1page", |b| {
        let pdf = Pdf::open(&simple, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.extract_words(&opts).len());
        });
    });

    group.bench_function("medium_10page", |b| {
        let pdf = Pdf::open(&medium, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.extract_words(&opts).len());
            }
        });
    });

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.extract_words(&opts).len());
            }
        });
    });

    group.finish();
}

fn bench_text_extraction(c: &mut Criterion) {
    let simple = simple_pdf_bytes();
    let medium = medium_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("text_extraction");
    let opts = TextOptions::default();

    group.bench_function("simple_1page", |b| {
        let pdf = Pdf::open(&simple, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.extract_text(&opts).len());
        });
    });

    group.bench_function("medium_10page", |b| {
        let pdf = Pdf::open(&medium, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.extract_text(&opts).len());
            }
        });
    });

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.extract_text(&opts).len());
            }
        });
    });

    group.finish();
}

fn bench_text_extraction_layout(c: &mut Criterion) {
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("text_extraction_layout");
    let opts = TextOptions {
        layout: true,
        ..TextOptions::default()
    };

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.extract_text(&opts).len());
            }
        });
    });

    group.finish();
}

fn bench_table_detection_lattice(c: &mut Criterion) {
    let lattice = lattice_table_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("table_detection_lattice");
    let settings = TableSettings::default(); // Lattice is default

    group.bench_function("20x5_single_table", |b| {
        let pdf = Pdf::open(&lattice, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.find_tables(&settings).len());
        });
    });

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.find_tables(&settings).len());
            }
        });
    });

    group.finish();
}

fn bench_table_detection_stream(c: &mut Criterion) {
    let stream = stream_table_pdf_bytes();

    let mut group = c.benchmark_group("table_detection_stream");
    let settings = TableSettings {
        strategy: Strategy::Stream,
        min_words_vertical: 2,
        min_words_horizontal: 1,
        ..TableSettings::default()
    };

    group.bench_function("20x5_single_table", |b| {
        let pdf = Pdf::open(&stream, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.find_tables(&settings).len());
        });
    });

    group.finish();
}

fn bench_pdf_open(c: &mut Criterion) {
    let simple = simple_pdf_bytes();
    let medium = medium_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("pdf_open");

    group.bench_function("simple_1page", |b| {
        b.iter(|| {
            let pdf = Pdf::open(black_box(&simple), None).unwrap();
            black_box(pdf.page_count());
        });
    });

    group.bench_function("medium_10page", |b| {
        b.iter(|| {
            let pdf = Pdf::open(black_box(&medium), None).unwrap();
            black_box(pdf.page_count());
        });
    });

    group.bench_function("complex_10page", |b| {
        b.iter(|| {
            let pdf = Pdf::open(black_box(&complex), None).unwrap();
            black_box(pdf.page_count());
        });
    });

    group.finish();
}

fn bench_edge_computation(c: &mut Criterion) {
    let lattice = lattice_table_pdf_bytes();
    let complex = complex_pdf_bytes();

    let mut group = c.benchmark_group("edge_computation");

    group.bench_function("lattice_20x5", |b| {
        let pdf = Pdf::open(&lattice, None).unwrap();
        b.iter(|| {
            let page = pdf.page(0).unwrap();
            black_box(page.edges().len());
        });
    });

    group.bench_function("complex_10page", |b| {
        let pdf = Pdf::open(&complex, None).unwrap();
        b.iter(|| {
            for i in 0..pdf.page_count() {
                let page = pdf.page(i).unwrap();
                black_box(page.edges().len());
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pdf_open,
    bench_char_extraction,
    bench_word_extraction,
    bench_text_extraction,
    bench_text_extraction_layout,
    bench_table_detection_lattice,
    bench_table_detection_stream,
    bench_edge_computation,
);
criterion_main!(benches);
