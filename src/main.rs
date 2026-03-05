mod cli;
mod scanner;

use clap::Parser;
use cli::{parse_token_count, Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Overview {
            tokens,
            verbose,
            path,
            ..
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if *verbose {
                eprintln!(
                    "cxpak: scanning {} with budget of {} tokens",
                    path.display(),
                    token_budget
                );
            }
            eprintln!("overview command not yet implemented");
        }
        Commands::Trace {
            tokens,
            target,
            verbose,
            ..
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if *verbose {
                eprintln!(
                    "cxpak: tracing '{}' with budget of {} tokens",
                    target, token_budget
                );
            }
            eprintln!("trace command not yet implemented");
        }
    }
}
