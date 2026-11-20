mod architecture;
mod cli;
mod esp;
mod genlist;
mod install;
mod pcrlock;
mod version;

use clap::Parser;

use cli::Cli;

fn main() {
    Cli::parse().call(module_path!())
}
