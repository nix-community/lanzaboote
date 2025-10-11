mod architecture;
mod cli;
mod esp;
mod install;

use clap::Parser;

use cli::Cli;

fn main() {
    Cli::parse().call(module_path!())
}
