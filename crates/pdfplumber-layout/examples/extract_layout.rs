//! Run semantic layout inference on a PDF and walk the structured result.
//!
//! Converts raw pdfplumber extraction into a [`Document`] with named
//! [`Section`]s, [`Heading`]s, [`Paragraph`]s, tables, and figures.
//! Header/footer zones are suppressed automatically (two-pass algorithm).
//!
//! Run with: `cargo run --example extract_layout -p pdfplumber-layout -- <file.pdf>`

use std::env;
use std::process::ExitCode;

use pdfplumber::Pdf;
use pdfplumber_layout::{Document, LayoutBlock};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: extract_layout <path-to-pdf>")?;
    let pdf  = Pdf::open_file(&path, None)?;
    let doc  = Document::from_pdf(&pdf);
    let s    = doc.stats();

    println!("=== {path} ===");
    println!("pages={} sections={} headings={} paragraphs={} tables={} figures={} words={}",
             s.page_count, s.section_count, s.heading_count,
             s.paragraph_count, s.table_count, s.figure_count,
             doc.word_count());
    println!();

    // ── Section walk ──────────────────────────────────────────────────────
    for (i, section) in doc.sections().iter().enumerate() {
        let heading = section.heading()
            .map(|h| format!("H{}: {}", h.level as u8, h.text()))
            .unwrap_or_else(|| "(untitled)".into());

        let n_para = section.paragraphs().len();
        println!("[§{}] {}  ({n_para} paragraph(s))", i + 1, heading);

        for para in section.paragraphs().iter().take(2) {
            let text = para.text();
            let preview = text.char_indices()
                .nth(80)
                .map(|(i, _)| &text[..i])
                .unwrap_or(text);
            let ellipsis = if text.len() > 80 { "…" } else { "" };
            println!("  ¶ {preview}{ellipsis}");
        }
        if n_para > 2 {
            println!("  … +{} more", n_para - 2);
        }
        println!();
    }

    // ── Block-type tally ──────────────────────────────────────────────────
    let (mut h, mut p, mut t, mut f) = (0usize, 0, 0, 0);
    for block in doc.blocks() {
        match block {
            LayoutBlock::Heading(_)   => h += 1,
            LayoutBlock::Paragraph(_) => p += 1,
            LayoutBlock::Table(_)     => t += 1,
            LayoutBlock::Figure(_)    => f += 1,
        }
    }
    println!("block breakdown → H:{h}  P:{p}  T:{t}  F:{f}");

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
