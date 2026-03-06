mod annots_cmd;
mod bookmarks_cmd;
mod chars_cmd;
mod cli;
mod debug_cmd;
mod forms_cmd;
mod images_cmd;
mod info_cmd;
mod links_cmd;
mod page_range;
mod search_cmd;
mod shared;
mod tables_cmd;
mod text_cmd;
mod validate_cmd;
mod words_cmd;

#[cfg(feature = "tui")]
mod tui;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    // TUI mode: `pdfplumber --tui [file]` — requires a TTY.
    // When `--no-tui` is passed, or stdout is not a TTY, fall through to
    // the regular subcommand dispatch.
    #[cfg(feature = "tui")]
    if let cli::Commands::Tui { ref file, ref dir, no_tui } = cli.command {
        if no_tui {
            eprintln!("pdfplumber: --no-tui flag set, falling back to headless mode");
            std::process::exit(0);
        }
        // Check we actually have a TTY on stderr (where ratatui draws)
        use std::io::IsTerminal;
        if !std::io::stderr().is_terminal() {
            eprintln!("pdfplumber: not a TTY, cannot launch TUI. Use headless subcommands.");
            std::process::exit(1);
        }
        if let Err(e) = tui::run(file.clone(), dir.clone()) {
            eprintln!("pdfplumber tui error: {e}");
            std::process::exit(1);
        }
        return;
    }

    let result = match cli.command {
        cli::Commands::Text {
            ref file,
            ref pages,
            ref format,
            layout,
            ref unicode_norm,
            ref password,
            repair,
        } => text_cmd::run(
            file,
            pages.as_deref(),
            format,
            layout,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
            repair,
        ),
        cli::Commands::Chars {
            ref file,
            ref pages,
            ref format,
            ref unicode_norm,
            ref password,
            repair,
        } => chars_cmd::run(
            file,
            pages.as_deref(),
            format,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
            repair,
        ),
        cli::Commands::Words {
            ref file,
            ref pages,
            ref format,
            x_tolerance,
            y_tolerance,
            ref unicode_norm,
            ref password,
            repair,
        } => words_cmd::run(
            file,
            pages.as_deref(),
            format,
            x_tolerance,
            y_tolerance,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
            repair,
        ),
        cli::Commands::Tables {
            ref file,
            ref pages,
            ref format,
            ref strategy,
            snap_tolerance,
            join_tolerance,
            text_tolerance,
            ref password,
            repair,
        } => tables_cmd::run(
            file,
            pages.as_deref(),
            format,
            strategy,
            snap_tolerance,
            join_tolerance,
            text_tolerance,
            password.as_deref(),
            repair,
        ),
        cli::Commands::Info {
            ref file,
            ref pages,
            ref format,
            ref password,
            repair,
        } => info_cmd::run(file, pages.as_deref(), format, password.as_deref(), repair),
        cli::Commands::Annots {
            ref file,
            ref pages,
            ref format,
            ref password,
            repair,
        } => annots_cmd::run(file, pages.as_deref(), format, password.as_deref(), repair),
        cli::Commands::Forms {
            ref file,
            ref pages,
            ref format,
            ref password,
            repair,
        } => forms_cmd::run(file, pages.as_deref(), format, password.as_deref(), repair),
        cli::Commands::Links {
            ref file,
            ref pages,
            ref format,
            ref password,
            repair,
        } => links_cmd::run(file, pages.as_deref(), format, password.as_deref(), repair),
        cli::Commands::Bookmarks {
            ref file,
            ref format,
            ref password,
            repair,
        } => bookmarks_cmd::run(file, format, password.as_deref(), repair),
        cli::Commands::Debug {
            ref file,
            ref pages,
            ref output,
            tables,
            ref password,
            repair,
        } => debug_cmd::run(
            file,
            pages.as_deref(),
            output,
            tables,
            password.as_deref(),
            repair,
        ),
        cli::Commands::Search {
            ref file,
            ref pattern,
            ref pages,
            case_insensitive,
            no_regex,
            ref format,
            ref password,
            repair,
        } => search_cmd::run(
            file,
            pattern,
            pages.as_deref(),
            case_insensitive,
            no_regex,
            format,
            password.as_deref(),
            repair,
        ),
        cli::Commands::Images {
            ref file,
            ref pages,
            ref format,
            extract,
            ref output_dir,
            ref password,
            repair,
        } => images_cmd::run(
            file,
            pages.as_deref(),
            format,
            extract,
            output_dir.as_deref(),
            password.as_deref(),
            repair,
        ),
        cli::Commands::Validate {
            ref file,
            ref format,
            ref password,
        } => validate_cmd::run(file, format, password.as_deref()),

        // Tui variant is handled above before the match; reaching here is
        // unreachable in practice, but required for exhaustive matching.
        #[cfg(feature = "tui")]
        cli::Commands::Tui { .. } => Ok(()),
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
