mod bootspec;
mod cli;
mod esp;
mod install;
mod pe;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() -> Result<()> {
    Cli::parse().call()
}
