mod chars_cmd;
mod cli;
mod page_range;
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
        cli::Commands::Tables { .. } => {
            eprintln!("tables subcommand not yet implemented");
            Err(1)
        }
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
