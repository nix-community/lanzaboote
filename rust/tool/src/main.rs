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

use clap::Parser;

use cli::Cli;

fn main() {
    Cli::parse().call(module_path!())
}
