//! Pure-Rust PDF page rasterizer.
//!
//! Renders a [`pdfplumber::Page`] (or any page with extracted chars, rects,
//! lines, curves) to a PNG image using [`tiny_skia`] for 2D rasterization and
//! [`fontdue`] for text glyph rendering.
//!
//! # Design goals
//!
//! - **Zero C dependencies.** `tiny-skia`, `fontdue`, and `png` are all pure Rust.
//! - **Sufficient fidelity for downstream use.** Rects, lines, text glyphs, and
//!   curves are rendered at correct positions and colors. Sub-pixel typography is
//!   not the goal — correctness of placement and approximate shape is.
//! - **Ollama-feedable output.** The primary downstream use case is feeding pages
//!   to a local vision model (Lane 7 ollama-fallback). The model needs a legible
//!   rendering, not typeset output.
//! - **WASM-compatible.** All deps are `no_std`-friendly or have std-only paths
//!   that can be disabled. `tiny-skia` with `default-features = false, features =
//!   ["std"]` compiles for `wasm32-unknown-unknown`.
//!
//! # Quick start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_raster::{Rasterizer, RasterOptions};
//!
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! let page = pdf.page(0).unwrap();
//!
//! let opts = RasterOptions { scale: 2.0, ..Default::default() };
//! let png_bytes = Rasterizer::new(opts).render_page(&page).unwrap();
//! std::fs::write("page0.png", &png_bytes).unwrap();
//! ```

#![deny(missing_docs)]

mod color;
mod font_cache;
mod render;

pub use render::{RasterError, RasterOptions, RenderResult, Rasterizer};
