//! `pdfplumber annotate` — add highlights, notes, and links to a PDF.
//!
//! Requires the `write` feature on the `pdfplumber` crate.
//! Without it, the subcommand reports an error.

use std::path::PathBuf;

#[cfg(feature = "write")]
use pdfplumber::BBox;

#[cfg(feature = "write")]
use pdfplumber::write::{AnnotationColor, PdfWriter};

/// Arguments for the `annotate` subcommand, bundled to avoid too-many-args.
#[allow(dead_code)]
pub struct AnnotateArgs<'a> {
    /// Input PDF path.
    pub input: &'a PathBuf,
    /// Output PDF path.
    pub output: &'a PathBuf,
    /// 1-based page number.
    pub page: usize,
    /// Bounding box: left.
    pub x0: f64,
    /// Bounding box: top.
    pub y0: f64,
    /// Bounding box: right.
    pub x1: f64,
    /// Bounding box: bottom.
    pub y1: f64,
    /// Whether to add a highlight annotation.
    pub highlight: bool,
    /// Optional text note content.
    pub text_note: Option<&'a str>,
    /// Optional link URI.
    pub link_uri: Option<&'a str>,
    /// Highlight color name.
    pub color: &'a str,
    /// Optional note contents for highlight.
    pub note_contents: Option<&'a str>,
    /// Optional PDF password.
    pub password: Option<&'a str>,
}

/// Run the `annotate` subcommand.
pub fn run(args: &AnnotateArgs<'_>) -> Result<(), i32> {
    #[cfg(not(feature = "write"))]
    {
        let _ = args;
        eprintln!("error: the `write` feature is not enabled. Rebuild with --features write");
        Err(1)
    }

    #[cfg(feature = "write")]
    {
        let file_bytes = std::fs::read(args.input).map_err(|e| {
            eprintln!("error reading {}: {e}", args.input.display());
            1i32
        })?;

        let pdf = crate::shared::open_pdf(args.input, args.password, false).map_err(|e| {
            eprintln!("error: {e}");
            1i32
        })?;

        let bbox = BBox {
            x0: args.x0,
            y0: args.y0,
            x1: args.x1,
            y1: args.y1,
        };
        let mut writer = PdfWriter::new(&pdf, &file_bytes);

        if args.highlight {
            let annot_color = parse_color(args.color);
            writer
                .add_highlight_with_comment(
                    args.page.saturating_sub(1),
                    bbox,
                    annot_color,
                    args.note_contents.unwrap_or(""),
                    "",
                )
                .map_err(|e| {
                    eprintln!("error adding highlight: {e}");
                    1i32
                })?;
        } else if let Some(text) = args.text_note {
            writer
                .add_text_annotation(args.page.saturating_sub(1), bbox, text)
                .map_err(|e| {
                    eprintln!("error adding text annotation: {e}");
                    1i32
                })?;
        } else if let Some(uri) = args.link_uri {
            writer
                .add_link_annotation(args.page.saturating_sub(1), bbox, uri)
                .map_err(|e| {
                    eprintln!("error adding link: {e}");
                    1i32
                })?;
        } else {
            eprintln!("error: specify --highlight, --text-note <text>, or --link-uri <url>");
            return Err(1);
        }

        let updated_bytes = writer.write_incremental().map_err(|e| {
            eprintln!("error writing incremental update: {e}");
            1i32
        })?;

        std::fs::write(args.output, &updated_bytes).map_err(|e| {
            eprintln!("error writing {}: {e}", args.output.display());
            1i32
        })?;

        println!(
            "annotated PDF written to {} ({} bytes, +{} bytes incremental)",
            args.output.display(),
            updated_bytes.len(),
            updated_bytes.len().saturating_sub(file_bytes.len()),
        );

        Ok(())
    }
}

#[cfg(feature = "write")]
fn parse_color(s: &str) -> AnnotationColor {
    match s.to_lowercase().as_str() {
        "yellow" => AnnotationColor::Yellow,
        "cyan" | "blue" => AnnotationColor::Cyan,
        "green" => AnnotationColor::Green,
        "pink" | "red" => AnnotationColor::Pink,
        _ => AnnotationColor::Yellow, // default
    }
}
