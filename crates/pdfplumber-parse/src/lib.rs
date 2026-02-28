//! pdfplumber-parse: PDF parsing backend and content stream interpreter.
//!
//! This crate implements Layer 1 (PDF parsing via pluggable backends) and
//! Layer 2 (content stream interpretation) of the pdfplumber-rs architecture.
//! It depends on pdfplumber-core for shared data types.

pub mod backend;
pub mod char_extraction;
pub mod cid_font;
pub mod cmap;
pub mod error;
pub mod font_metrics;
pub mod handler;
pub mod interpreter;
pub mod interpreter_state;
pub mod lopdf_backend;
pub mod page_geometry;
pub mod text_renderer;
pub mod text_state;
pub mod tokenizer;

pub use backend::PdfBackend;
pub use char_extraction::char_from_event;
pub use cid_font::{
    CidFontMetrics, CidFontType, CidSystemInfo, CidToGidMap, PredefinedCMapInfo,
    extract_cid_font_metrics, get_descendant_font, get_type0_encoding, is_type0_font,
    parse_predefined_cmap_name, parse_w_array,
};
pub use cmap::{CMap, CidCMap};
pub use error::BackendError;
pub use font_metrics::{FontMetrics, extract_font_metrics};
pub use handler::{CharEvent, ContentHandler, ImageEvent, PaintOp, PathEvent};
pub use interpreter_state::InterpreterState;
pub use lopdf_backend::{LopdfBackend, LopdfDocument, LopdfPage};
pub use page_geometry::PageGeometry;
pub use pdfplumber_core;
pub use text_renderer::{
    RawChar, TjElement, double_quote_show_string, quote_show_string, show_string,
    show_string_with_positioning,
};
pub use text_state::{TextRenderMode, TextState};
pub use tokenizer::{Operand, Operator, tokenize};
