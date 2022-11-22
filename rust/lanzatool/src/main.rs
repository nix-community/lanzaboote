mod cli;
mod crypto;

use std::process;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Err(e) = cli.call() {
        eprintln!("{}", e);
        process::exit(1)
    };
    Ok(())
}
