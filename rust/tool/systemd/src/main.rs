mod architecture;
mod cli;
mod esp;
mod install;
mod version;

use clap::Parser;

use cli::Cli;

fn main() {
    Cli::parse().call(module_path!())
}
