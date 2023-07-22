mod cli;
mod esp;
mod install;

use cli::Cli;
use clap::Parser;

fn main() {
    Cli::parse().call(module_path!())
}
