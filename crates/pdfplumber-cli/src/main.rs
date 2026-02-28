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
            ref password,
        } => text_cmd::run(
            file,
            pages.as_deref(),
            format,
            layout,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
        ),
        cli::Commands::Chars {
            ref file,
            ref pages,
            ref format,
            ref unicode_norm,
            ref password,
        } => chars_cmd::run(
            file,
            pages.as_deref(),
            format,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
        ),
        cli::Commands::Words {
            ref file,
            ref pages,
            ref format,
            x_tolerance,
            y_tolerance,
            ref unicode_norm,
            ref password,
        } => words_cmd::run(
            file,
            pages.as_deref(),
            format,
            x_tolerance,
            y_tolerance,
            unicode_norm.as_ref().map(|n| n.to_unicode_norm()),
            password.as_deref(),
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
        } => tables_cmd::run(
            file,
            pages.as_deref(),
            format,
            strategy,
            snap_tolerance,
            join_tolerance,
            text_tolerance,
            password.as_deref(),
        ),
        cli::Commands::Info {
            ref file,
            ref pages,
            ref format,
            ref password,
        } => info_cmd::run(file, pages.as_deref(), format, password.as_deref()),
        cli::Commands::Annots {
            ref file,
            ref pages,
            ref format,
            ref password,
        } => annots_cmd::run(file, pages.as_deref(), format, password.as_deref()),
        cli::Commands::Forms {
            ref file,
            ref pages,
            ref format,
            ref password,
        } => forms_cmd::run(file, pages.as_deref(), format, password.as_deref()),
        cli::Commands::Links {
            ref file,
            ref pages,
            ref format,
            ref password,
        } => links_cmd::run(file, pages.as_deref(), format, password.as_deref()),
        cli::Commands::Bookmarks {
            ref file,
            ref format,
            ref password,
        } => bookmarks_cmd::run(file, format, password.as_deref()),
        cli::Commands::Debug {
            ref file,
            ref pages,
            ref output,
            tables,
            ref password,
        } => debug_cmd::run(file, pages.as_deref(), output, tables, password.as_deref()),
        cli::Commands::Search {
            ref file,
            ref pattern,
            ref pages,
            case_insensitive,
            no_regex,
            ref format,
            ref password,
        } => search_cmd::run(
            file,
            pattern,
            pages.as_deref(),
            case_insensitive,
            no_regex,
            format,
            password.as_deref(),
        ),
        cli::Commands::Images {
            ref file,
            ref pages,
            ref format,
            extract,
            ref output_dir,
            ref password,
        } => images_cmd::run(
            file,
            pages.as_deref(),
            format,
            extract,
            output_dir.as_deref(),
            password.as_deref(),
        ),
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
