mod check;
mod cli;
mod diff;
mod edit;
mod edition;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::run(cli) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(n) => ExitCode::from(n as u8),
        Err(e) => {
            eprintln!("エラー: {e}");
            ExitCode::FAILURE
        }
    }
}
