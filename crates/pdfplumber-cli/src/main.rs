mod annots_cmd;
mod bookmarks_cmd;
mod chars_cmd;
mod cli;
mod debug_cmd;
mod info_cmd;
mod links_cmd;
mod page_range;
mod search_cmd;
mod shared;
mod tables_cmd;
mod text_cmd;
mod words_cmd;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        cli::Commands::Text {
            ref file,
            ref pages,
            ref format,
            layout,
            ref unicode_norm,
        } => text_cmd::run(
            file,
            pages.as_deref(),
            format,
            layout,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
        ),
        cli::Commands::Chars {
            ref file,
            ref pages,
            ref format,
            ref unicode_norm,
        } => chars_cmd::run(
            file,
            pages.as_deref(),
            format,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
        ),
        cli::Commands::Words {
            ref file,
            ref pages,
            ref format,
            x_tolerance,
            y_tolerance,
            ref unicode_norm,
        } => words_cmd::run(
            file,
            pages.as_deref(),
            format,
            x_tolerance,
            y_tolerance,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
        ),
        cli::Commands::Tables {
            ref file,
            ref pages,
            ref format,
            ref strategy,
            snap_tolerance,
            join_tolerance,
            text_tolerance,
        } => tables_cmd::run(
            file,
            pages.as_deref(),
            format,
            strategy,
            snap_tolerance,
            join_tolerance,
            text_tolerance,
        ),
        cli::Commands::Info {
            ref file,
            ref pages,
            ref format,
        } => info_cmd::run(file, pages.as_deref(), format),
        cli::Commands::Annots {
            ref file,
            ref pages,
            ref format,
        } => annots_cmd::run(file, pages.as_deref(), format),
        cli::Commands::Links {
            ref file,
            ref pages,
            ref format,
        } => links_cmd::run(file, pages.as_deref(), format),
        cli::Commands::Bookmarks {
            ref file,
            ref format,
        } => bookmarks_cmd::run(file, format),
        cli::Commands::Debug {
            ref file,
            ref pages,
            ref output,
            tables,
        } => debug_cmd::run(file, pages.as_deref(), output, tables),
        cli::Commands::Search {
            ref file,
            ref pattern,
            ref pages,
            case_insensitive,
            no_regex,
            ref format,
        } => search_cmd::run(
            file,
            pattern,
            pages.as_deref(),
            case_insensitive,
            no_regex,
            format,
        ),
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
