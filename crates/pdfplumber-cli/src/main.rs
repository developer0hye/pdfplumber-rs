mod cli;

use clap::Parser;
use cli::Cli;

fn main() {
    let _cli = Cli::parse();

    match _cli.command {
        cli::Commands::Text { .. } => {
            eprintln!("text subcommand not yet implemented");
            std::process::exit(1);
        }
        cli::Commands::Chars { .. } => {
            eprintln!("chars subcommand not yet implemented");
            std::process::exit(1);
        }
        cli::Commands::Words { .. } => {
            eprintln!("words subcommand not yet implemented");
            std::process::exit(1);
        }
        cli::Commands::Tables { .. } => {
            eprintln!("tables subcommand not yet implemented");
            std::process::exit(1);
        }
    }
}
