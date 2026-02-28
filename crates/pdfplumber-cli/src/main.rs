mod cli;
mod page_range;
mod text_cmd;

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
        cli::Commands::Chars { .. } => {
            eprintln!("chars subcommand not yet implemented");
            Err(1)
        }
        cli::Commands::Words { .. } => {
            eprintln!("words subcommand not yet implemented");
            Err(1)
        }
        cli::Commands::Tables { .. } => {
            eprintln!("tables subcommand not yet implemented");
            Err(1)
        }
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
