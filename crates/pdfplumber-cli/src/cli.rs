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
    },
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
        ]);
        match cli.command {
            Commands::Tables {
                ref file,
                ref pages,
                ref format,
            } => {
                assert_eq!(file, &PathBuf::from("doc.pdf"));
                assert_eq!(pages.as_deref(), Some("2-4"));
                assert!(matches!(format, OutputFormat::Json));
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
}
