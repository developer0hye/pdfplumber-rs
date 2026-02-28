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
    },

    /// Extract bookmarks (outline / table of contents) from PDF
    Bookmarks {
        /// Path to the PDF file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = TextFormat::Text)]
        format: TextFormat,
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
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_text_subcommand_with_file() {
        let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf"]);
        match cli.command {
            Commands::Text { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn parse_text_with_pages_and_format() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "text",
            "test.pdf",
            "--pages",
            "1,3-5",
            "--format",
            "json",
        ]);
        match cli.command {
            Commands::Text {
                ref file,
                ref pages,
                ref format,
                layout,
                ..
            } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
                assert_eq!(pages.as_deref(), Some("1,3-5"));
                assert!(matches!(format, TextFormat::Json));
                assert!(!layout);
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn parse_text_with_layout_flag() {
        let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf", "--layout"]);
        match cli.command {
            Commands::Text { layout, .. } => {
                assert!(layout);
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn parse_chars_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "chars", "input.pdf"]);
        match cli.command {
            Commands::Chars { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("input.pdf"));
            }
            _ => panic!("expected Chars subcommand"),
        }
    }

    #[test]
    fn parse_chars_with_csv_format() {
        let cli = Cli::parse_from(["pdfplumber", "chars", "input.pdf", "--format", "csv"]);
        match cli.command {
            Commands::Chars { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Csv));
            }
            _ => panic!("expected Chars subcommand"),
        }
    }

    #[test]
    fn parse_words_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "words", "test.pdf"]);
        match cli.command {
            Commands::Words { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Words subcommand"),
        }
    }

    #[test]
    fn parse_words_with_tolerance_options() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "words",
            "test.pdf",
            "--x-tolerance",
            "5.0",
            "--y-tolerance",
            "2.5",
        ]);
        match cli.command {
            Commands::Words {
                x_tolerance,
                y_tolerance,
                ..
            } => {
                assert!((x_tolerance - 5.0).abs() < f64::EPSILON);
                assert!((y_tolerance - 2.5).abs() < f64::EPSILON);
            }
            _ => panic!("expected Words subcommand"),
        }
    }

    #[test]
    fn parse_words_default_tolerances() {
        let cli = Cli::parse_from(["pdfplumber", "words", "test.pdf"]);
        match cli.command {
            Commands::Words {
                x_tolerance,
                y_tolerance,
                ..
            } => {
                assert!((x_tolerance - 3.0).abs() < f64::EPSILON);
                assert!((y_tolerance - 3.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Words subcommand"),
        }
    }

    #[test]
    fn parse_tables_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "tables", "test.pdf"]);
        match cli.command {
            Commands::Tables { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Tables subcommand"),
        }
    }

    #[test]
    fn parse_tables_with_all_options() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "tables",
            "doc.pdf",
            "--pages",
            "2-4",
            "--format",
            "json",
            "--strategy",
            "stream",
            "--snap-tolerance",
            "5.0",
            "--join-tolerance",
            "4.0",
            "--text-tolerance",
            "2.0",
        ]);
        match cli.command {
            Commands::Tables {
                ref file,
                ref pages,
                ref format,
                ref strategy,
                snap_tolerance,
                join_tolerance,
                text_tolerance,
            } => {
                assert_eq!(file, &PathBuf::from("doc.pdf"));
                assert_eq!(pages.as_deref(), Some("2-4"));
                assert!(matches!(format, OutputFormat::Json));
                assert!(matches!(strategy, TableStrategy::Stream));
                assert!((snap_tolerance - 5.0).abs() < f64::EPSILON);
                assert!((join_tolerance - 4.0).abs() < f64::EPSILON);
                assert!((text_tolerance - 2.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Tables subcommand"),
        }
    }

    #[test]
    fn parse_tables_default_strategy_and_tolerances() {
        let cli = Cli::parse_from(["pdfplumber", "tables", "test.pdf"]);
        match cli.command {
            Commands::Tables {
                ref strategy,
                snap_tolerance,
                join_tolerance,
                text_tolerance,
                ..
            } => {
                assert!(matches!(strategy, TableStrategy::Lattice));
                assert!((snap_tolerance - 3.0).abs() < f64::EPSILON);
                assert!((join_tolerance - 3.0).abs() < f64::EPSILON);
                assert!((text_tolerance - 3.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Tables subcommand"),
        }
    }

    #[test]
    fn text_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf"]);
        match cli.command {
            Commands::Text { ref format, .. } => {
                assert!(matches!(format, TextFormat::Text));
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn chars_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "chars", "test.pdf"]);
        match cli.command {
            Commands::Chars { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Text));
            }
            _ => panic!("expected Chars subcommand"),
        }
    }

    #[test]
    fn parse_info_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "info", "test.pdf"]);
        match cli.command {
            Commands::Info { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Info subcommand"),
        }
    }

    #[test]
    fn parse_info_with_json_format() {
        let cli = Cli::parse_from(["pdfplumber", "info", "test.pdf", "--format", "json"]);
        match cli.command {
            Commands::Info { ref format, .. } => {
                assert!(matches!(format, TextFormat::Json));
            }
            _ => panic!("expected Info subcommand"),
        }
    }

    #[test]
    fn parse_info_with_pages() {
        let cli = Cli::parse_from(["pdfplumber", "info", "test.pdf", "--pages", "1-3"]);
        match cli.command {
            Commands::Info { ref pages, .. } => {
                assert_eq!(pages.as_deref(), Some("1-3"));
            }
            _ => panic!("expected Info subcommand"),
        }
    }

    #[test]
    fn info_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "info", "test.pdf"]);
        match cli.command {
            Commands::Info { ref format, .. } => {
                assert!(matches!(format, TextFormat::Text));
            }
            _ => panic!("expected Info subcommand"),
        }
    }

    #[test]
    fn parse_annots_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "annots", "test.pdf"]);
        match cli.command {
            Commands::Annots { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Annots subcommand"),
        }
    }

    #[test]
    fn parse_annots_with_json_format() {
        let cli = Cli::parse_from(["pdfplumber", "annots", "test.pdf", "--format", "json"]);
        match cli.command {
            Commands::Annots { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Json));
            }
            _ => panic!("expected Annots subcommand"),
        }
    }

    #[test]
    fn annots_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "annots", "test.pdf"]);
        match cli.command {
            Commands::Annots { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Text));
            }
            _ => panic!("expected Annots subcommand"),
        }
    }

    #[test]
    fn parse_links_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "links", "test.pdf"]);
        match cli.command {
            Commands::Links { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Links subcommand"),
        }
    }

    #[test]
    fn parse_links_with_csv_format() {
        let cli = Cli::parse_from(["pdfplumber", "links", "test.pdf", "--format", "csv"]);
        match cli.command {
            Commands::Links { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Csv));
            }
            _ => panic!("expected Links subcommand"),
        }
    }

    #[test]
    fn links_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "links", "test.pdf"]);
        match cli.command {
            Commands::Links { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Text));
            }
            _ => panic!("expected Links subcommand"),
        }
    }

    #[test]
    fn parse_bookmarks_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "bookmarks", "test.pdf"]);
        match cli.command {
            Commands::Bookmarks { ref file, .. } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
            }
            _ => panic!("expected Bookmarks subcommand"),
        }
    }

    #[test]
    fn parse_bookmarks_with_json_format() {
        let cli = Cli::parse_from(["pdfplumber", "bookmarks", "test.pdf", "--format", "json"]);
        match cli.command {
            Commands::Bookmarks { ref format, .. } => {
                assert!(matches!(format, TextFormat::Json));
            }
            _ => panic!("expected Bookmarks subcommand"),
        }
    }

    #[test]
    fn bookmarks_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "bookmarks", "test.pdf"]);
        match cli.command {
            Commands::Bookmarks { ref format, .. } => {
                assert!(matches!(format, TextFormat::Text));
            }
            _ => panic!("expected Bookmarks subcommand"),
        }
    }

    #[test]
    fn parse_search_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "search", "test.pdf", "hello"]);
        match cli.command {
            Commands::Search {
                ref file,
                ref pattern,
                case_insensitive,
                no_regex,
                ..
            } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
                assert_eq!(pattern, "hello");
                assert!(!case_insensitive);
                assert!(!no_regex);
            }
            _ => panic!("expected Search subcommand"),
        }
    }

    #[test]
    fn parse_search_with_options() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "search",
            "test.pdf",
            "pattern",
            "--case-insensitive",
            "--no-regex",
            "--pages",
            "1,3-5",
            "--format",
            "json",
        ]);
        match cli.command {
            Commands::Search {
                ref pattern,
                ref pages,
                case_insensitive,
                no_regex,
                ref format,
                ..
            } => {
                assert_eq!(pattern, "pattern");
                assert_eq!(pages.as_deref(), Some("1,3-5"));
                assert!(case_insensitive);
                assert!(no_regex);
                assert!(matches!(format, OutputFormat::Json));
            }
            _ => panic!("expected Search subcommand"),
        }
    }

    #[test]
    fn search_default_format_is_text() {
        let cli = Cli::parse_from(["pdfplumber", "search", "test.pdf", "query"]);
        match cli.command {
            Commands::Search { ref format, .. } => {
                assert!(matches!(format, OutputFormat::Text));
            }
            _ => panic!("expected Search subcommand"),
        }
    }

    #[test]
    fn parse_text_with_unicode_norm_nfc() {
        let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf", "--unicode-norm", "nfc"]);
        match cli.command {
            Commands::Text {
                ref unicode_norm, ..
            } => {
                assert!(matches!(unicode_norm, Some(UnicodeNormArg::Nfc)));
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn parse_text_without_unicode_norm() {
        let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf"]);
        match cli.command {
            Commands::Text {
                ref unicode_norm, ..
            } => {
                assert!(unicode_norm.is_none());
            }
            _ => panic!("expected Text subcommand"),
        }
    }

    #[test]
    fn parse_chars_with_unicode_norm_nfkc() {
        let cli = Cli::parse_from(["pdfplumber", "chars", "test.pdf", "--unicode-norm", "nfkc"]);
        match cli.command {
            Commands::Chars {
                ref unicode_norm, ..
            } => {
                assert!(matches!(unicode_norm, Some(UnicodeNormArg::Nfkc)));
            }
            _ => panic!("expected Chars subcommand"),
        }
    }

    #[test]
    fn parse_words_with_unicode_norm_nfkd() {
        let cli = Cli::parse_from(["pdfplumber", "words", "test.pdf", "--unicode-norm", "nfkd"]);
        match cli.command {
            Commands::Words {
                ref unicode_norm, ..
            } => {
                assert!(matches!(unicode_norm, Some(UnicodeNormArg::Nfkd)));
            }
            _ => panic!("expected Words subcommand"),
        }
    }

    #[test]
    fn parse_debug_subcommand() {
        let cli = Cli::parse_from(["pdfplumber", "debug", "test.pdf", "--output", "out.svg"]);
        match cli.command {
            Commands::Debug {
                ref file,
                ref pages,
                ref output,
                tables,
            } => {
                assert_eq!(file, &PathBuf::from("test.pdf"));
                assert!(pages.is_none());
                assert_eq!(output, &PathBuf::from("out.svg"));
                assert!(!tables);
            }
            _ => panic!("expected Debug subcommand"),
        }
    }

    #[test]
    fn parse_debug_with_tables_flag() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "debug",
            "test.pdf",
            "--tables",
            "--output",
            "out.svg",
        ]);
        match cli.command {
            Commands::Debug { tables, .. } => {
                assert!(tables);
            }
            _ => panic!("expected Debug subcommand"),
        }
    }

    #[test]
    fn parse_debug_with_pages() {
        let cli = Cli::parse_from([
            "pdfplumber",
            "debug",
            "test.pdf",
            "--pages",
            "1-3",
            "--output",
            "debug.svg",
        ]);
        match cli.command {
            Commands::Debug {
                ref pages,
                ref output,
                ..
            } => {
                assert_eq!(pages.as_deref(), Some("1-3"));
                assert_eq!(output, &PathBuf::from("debug.svg"));
            }
            _ => panic!("expected Debug subcommand"),
        }
    }

    #[test]
    fn unicode_norm_arg_to_unicode_norm_all_variants() {
        assert_eq!(
            UnicodeNormArg::Nfc.to_unicode_norm(),
            pdfplumber::UnicodeNorm::Nfc
        );
        assert_eq!(
            UnicodeNormArg::Nfd.to_unicode_norm(),
            pdfplumber::UnicodeNorm::Nfd
        );
        assert_eq!(
            UnicodeNormArg::Nfkc.to_unicode_norm(),
            pdfplumber::UnicodeNorm::Nfkc
        );
        assert_eq!(
            UnicodeNormArg::Nfkd.to_unicode_norm(),
            pdfplumber::UnicodeNorm::Nfkd
        );
    }
}
