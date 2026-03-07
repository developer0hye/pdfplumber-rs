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
            ..
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
            ..
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

#[test]
fn parse_images_subcommand() {
    let cli = Cli::parse_from(["pdfplumber", "images", "test.pdf"]);
    match cli.command {
        Commands::Images {
            ref file,
            extract,
            ref output_dir,
            ..
        } => {
            assert_eq!(file, &PathBuf::from("test.pdf"));
            assert!(!extract);
            assert!(output_dir.is_none());
        }
        _ => panic!("expected Images subcommand"),
    }
}

#[test]
fn parse_images_with_extract_and_output_dir() {
    let cli = Cli::parse_from([
        "pdfplumber",
        "images",
        "test.pdf",
        "--extract",
        "--output-dir",
        "/tmp/images",
    ]);
    match cli.command {
        Commands::Images {
            extract,
            ref output_dir,
            ..
        } => {
            assert!(extract);
            assert_eq!(
                output_dir.as_deref(),
                Some(std::path::Path::new("/tmp/images"))
            );
        }
        _ => panic!("expected Images subcommand"),
    }
}

#[test]
fn parse_images_with_json_format() {
    let cli = Cli::parse_from(["pdfplumber", "images", "test.pdf", "--format", "json"]);
    match cli.command {
        Commands::Images { ref format, .. } => {
            assert!(matches!(format, OutputFormat::Json));
        }
        _ => panic!("expected Images subcommand"),
    }
}

#[test]
fn images_default_format_is_text() {
    let cli = Cli::parse_from(["pdfplumber", "images", "test.pdf"]);
    match cli.command {
        Commands::Images { ref format, .. } => {
            assert!(matches!(format, OutputFormat::Text));
        }
        _ => panic!("expected Images subcommand"),
    }
}

// --- Password flag tests ---

#[test]
fn parse_text_with_password() {
    let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf", "--password", "secret123"]);
    match cli.command {
        Commands::Text { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("secret123"));
        }
        _ => panic!("expected Text subcommand"),
    }
}

#[test]
fn parse_text_without_password() {
    let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf"]);
    match cli.command {
        Commands::Text { ref password, .. } => {
            assert!(password.is_none());
        }
        _ => panic!("expected Text subcommand"),
    }
}

#[test]
fn parse_info_with_password() {
    let cli = Cli::parse_from(["pdfplumber", "info", "test.pdf", "--password", "mypass"]);
    match cli.command {
        Commands::Info { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("mypass"));
        }
        _ => panic!("expected Info subcommand"),
    }
}

#[test]
fn parse_tables_with_password() {
    let cli = Cli::parse_from(["pdfplumber", "tables", "test.pdf", "--password", "pw"]);
    match cli.command {
        Commands::Tables { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("pw"));
        }
        _ => panic!("expected Tables subcommand"),
    }
}

#[test]
fn parse_search_with_password() {
    let cli = Cli::parse_from([
        "pdfplumber",
        "search",
        "test.pdf",
        "pattern",
        "--password",
        "pw",
    ]);
    match cli.command {
        Commands::Search { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("pw"));
        }
        _ => panic!("expected Search subcommand"),
    }
}

// --- Forms subcommand tests ---

#[test]
fn parse_forms_subcommand() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf"]);
    match cli.command {
        Commands::Forms { ref file, .. } => {
            assert_eq!(file, &PathBuf::from("test.pdf"));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

#[test]
fn parse_forms_with_json_format() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf", "--format", "json"]);
    match cli.command {
        Commands::Forms { ref format, .. } => {
            assert!(matches!(format, OutputFormat::Json));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

#[test]
fn parse_forms_with_csv_format() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf", "--format", "csv"]);
    match cli.command {
        Commands::Forms { ref format, .. } => {
            assert!(matches!(format, OutputFormat::Csv));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

#[test]
fn forms_default_format_is_text() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf"]);
    match cli.command {
        Commands::Forms { ref format, .. } => {
            assert!(matches!(format, OutputFormat::Text));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

#[test]
fn parse_forms_with_pages() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf", "--pages", "1-3"]);
    match cli.command {
        Commands::Forms { ref pages, .. } => {
            assert_eq!(pages.as_deref(), Some("1-3"));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

#[test]
fn parse_forms_with_password() {
    let cli = Cli::parse_from(["pdfplumber", "forms", "test.pdf", "--password", "secret"]);
    match cli.command {
        Commands::Forms { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("secret"));
        }
        _ => panic!("expected Forms subcommand"),
    }
}

// --- Validate subcommand tests ---

#[test]
fn parse_validate_subcommand() {
    let cli = Cli::parse_from(["pdfplumber", "validate", "test.pdf"]);
    match cli.command {
        Commands::Validate { ref file, .. } => {
            assert_eq!(file, &PathBuf::from("test.pdf"));
        }
        _ => panic!("expected Validate subcommand"),
    }
}

#[test]
fn validate_default_format_is_text() {
    let cli = Cli::parse_from(["pdfplumber", "validate", "test.pdf"]);
    match cli.command {
        Commands::Validate { ref format, .. } => {
            assert!(matches!(format, ValidateFormat::Text));
        }
        _ => panic!("expected Validate subcommand"),
    }
}

#[test]
fn parse_validate_with_json_format() {
    let cli = Cli::parse_from(["pdfplumber", "validate", "test.pdf", "--format", "json"]);
    match cli.command {
        Commands::Validate { ref format, .. } => {
            assert!(matches!(format, ValidateFormat::Json));
        }
        _ => panic!("expected Validate subcommand"),
    }
}

#[test]
fn parse_validate_with_password() {
    let cli = Cli::parse_from(["pdfplumber", "validate", "test.pdf", "--password", "secret"]);
    match cli.command {
        Commands::Validate { ref password, .. } => {
            assert_eq!(password.as_deref(), Some("secret"));
        }
        _ => panic!("expected Validate subcommand"),
    }
}

// --- Repair flag tests ---

#[test]
fn parse_text_with_repair_flag() {
    let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf", "--repair"]);
    match cli.command {
        Commands::Text { repair, .. } => {
            assert!(repair);
        }
        _ => panic!("expected Text subcommand"),
    }
}

#[test]
fn parse_text_without_repair_flag() {
    let cli = Cli::parse_from(["pdfplumber", "text", "test.pdf"]);
    match cli.command {
        Commands::Text { repair, .. } => {
            assert!(!repair);
        }
        _ => panic!("expected Text subcommand"),
    }
}

#[test]
fn parse_chars_with_repair_flag() {
    let cli = Cli::parse_from(["pdfplumber", "chars", "test.pdf", "--repair"]);
    match cli.command {
        Commands::Chars { repair, .. } => {
            assert!(repair);
        }
        _ => panic!("expected Chars subcommand"),
    }
}

#[test]
fn parse_tables_with_repair_flag() {
    let cli = Cli::parse_from(["pdfplumber", "tables", "test.pdf", "--repair"]);
    match cli.command {
        Commands::Tables { repair, .. } => {
            assert!(repair);
        }
        _ => panic!("expected Tables subcommand"),
    }
}
