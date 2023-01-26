mod cli;
mod esp;
mod gc;
mod generation;
mod install;
mod os_release;
mod pe;
mod signature;
mod systemd;
mod utils;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

fn main() -> Result<()> {
    Cli::parse().call()
}
