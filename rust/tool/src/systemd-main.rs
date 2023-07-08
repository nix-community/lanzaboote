mod systemd;
pub mod common;

use clap::Parser;

fn main() {
    systemd::Cli::parse().call(module_path!())
}
