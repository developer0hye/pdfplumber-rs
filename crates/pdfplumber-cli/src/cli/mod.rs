use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Extract text, characters, words, and tables from PDF documents.
#[derive(Debug, Parser)]
#[command(name = "pdfplumber", about, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Extract text from PDF pages
    Text {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = TextFormat::Text)]
        format: TextFormat,

        /// Use layout-preserving text extraction
        #[arg(long)]
        layout: bool,

        /// Apply Unicode normalization to extracted text
        #[arg(long, value_enum)]
        unicode_norm: Option<UnicodeNormArg>,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract individual characters with coordinates
    Chars {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Apply Unicode normalization to extracted text
        #[arg(long, value_enum)]
        unicode_norm: Option<UnicodeNormArg>,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract words with bounding box coordinates
    Words {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Horizontal tolerance for word grouping (default: 3.0)
        #[arg(long, default_value_t = 3.0)]
        x_tolerance: f64,

        /// Vertical tolerance for word grouping (default: 3.0)
        #[arg(long, default_value_t = 3.0)]
        y_tolerance: f64,

        /// Apply Unicode normalization to extracted text
        #[arg(long, value_enum)]
        unicode_norm: Option<UnicodeNormArg>,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Detect and extract tables from PDF pages
    Tables {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Table detection strategy
        #[arg(long, value_enum, default_value_t = TableStrategy::Lattice)]
        strategy: TableStrategy,

        /// Snap tolerance for aligning nearby edges (default: 3.0)
        #[arg(long, default_value_t = 3.0)]
        snap_tolerance: f64,

        /// Join tolerance for merging collinear edges (default: 3.0)
        #[arg(long, default_value_t = 3.0)]
        join_tolerance: f64,

        /// Text tolerance for assigning text to cells (default: 3.0)
        #[arg(long, default_value_t = 3.0)]
        text_tolerance: f64,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Display PDF metadata and page information
    Info {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = TextFormat::Text)]
        format: TextFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract annotations from PDF pages
    Annots {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract hyperlinks from PDF pages
    Links {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract bookmarks (outline / table of contents) from PDF
    Bookmarks {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = TextFormat::Text)]
        format: TextFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Extract form fields from PDF pages
    Forms {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Generate debug SVG with object overlays
    Debug {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output SVG file path
        #[arg(long, value_name = "FILE")]
        output: PathBuf,

        /// Show table detection pipeline (edges, intersections, cells, tables)
        #[arg(long)]
        tables: bool,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Search for text patterns with position information
    Search {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Search pattern (regex by default, use --no-regex for literal)
        #[arg(value_name = "PATTERN")]
        pattern: String,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Disable case sensitivity
        #[arg(long)]
        case_insensitive: bool,

        /// Treat pattern as literal string (not regex)
        #[arg(long)]
        no_regex: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// List or extract images from PDF pages
    Images {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Page range (e.g. '1,3-5'). Default: all pages
        #[arg(long)]
        pages: Option<String>,

        /// Output format (for listing mode)
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,

        /// Extract image content and save to disk
        #[arg(long)]
        extract: bool,

        /// Output directory for extracted images (default: current directory)
        #[arg(long, value_name = "DIR")]
        output_dir: Option<PathBuf>,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,

        /// Attempt best-effort repair before extraction
        #[arg(long)]
        repair: bool,
    },

    /// Validate PDF structure and report specification violations
    Validate {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = ValidateFormat::Text)]
        format: ValidateFormat,

        /// Password for encrypted PDFs
        #[arg(long)]
        password: Option<String>,
    },
}

/// Table detection strategy.
#[derive(Debug, Clone, ValueEnum)]
pub enum TableStrategy {
    /// Detect tables using visible lines and rect edges
    Lattice,
    /// Detect tables from text alignment patterns
    Stream,
}

/// Output format for text subcommand.
#[derive(Debug, Clone, ValueEnum)]
pub enum TextFormat {
    /// Plain text output
    Text,
    /// JSON output
    Json,
    /// HTML output (semantic HTML with headings, paragraphs, tables, emphasis)
    Html,
}

/// Output format for chars/words/tables subcommands.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Plain text (tab-separated)
    Text,
    /// JSON output
    Json,
    /// CSV output
    Csv,
}

/// Output format for validate subcommand.
#[derive(Debug, Clone, ValueEnum)]
pub enum ValidateFormat {
    /// Plain text output
    Text,
    /// JSON output
    Json,
}

/// Unicode normalization form for CLI arguments.
#[derive(Debug, Clone, ValueEnum)]
pub enum UnicodeNormArg {
    /// Canonical Decomposition, followed by Canonical Composition
    Nfc,
    /// Canonical Decomposition
    Nfd,
    /// Compatibility Decomposition, followed by Canonical Composition
    Nfkc,
    /// Compatibility Decomposition
    Nfkd,
}

impl UnicodeNormArg {
    /// Convert to the core library's `UnicodeNorm` enum.
    pub fn to_unicode_norm(&self) -> pdfplumber::UnicodeNorm {
        match self {
            UnicodeNormArg::Nfc => pdfplumber::UnicodeNorm::Nfc,
            UnicodeNormArg::Nfd => pdfplumber::UnicodeNorm::Nfd,
            UnicodeNormArg::Nfkc => pdfplumber::UnicodeNorm::Nfkc,
            UnicodeNormArg::Nfkd => pdfplumber::UnicodeNorm::Nfkd,
        }
    }
}


#[cfg(test)]
mod tests;
