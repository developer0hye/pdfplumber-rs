mod annots_cmd;
mod bookmarks_cmd;
mod chars_cmd;
mod cli;
mod info_cmd;
mod links_cmd;
mod page_range;
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
        } => text_cmd::run(file, pages.as_deref(), format, layout),
        cli::Commands::Chars {
            ref file,
            ref pages,
            ref format,
        } => chars_cmd::run(file, pages.as_deref(), format),
        cli::Commands::Words {
            ref file,
            ref pages,
            ref format,
            x_tolerance,
            y_tolerance,
        } => words_cmd::run(file, pages.as_deref(), format, x_tolerance, y_tolerance),
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
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
