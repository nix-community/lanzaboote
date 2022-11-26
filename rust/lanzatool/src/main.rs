mod bootspec;
mod cli;
mod esp;
mod generation;
mod install;
mod pe;
mod signer;
mod utils;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() -> Result<()> {
    Cli::parse().call()
}
