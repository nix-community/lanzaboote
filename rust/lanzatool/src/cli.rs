use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::install;

#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Install(InstallCommand),
}

#[derive(Parser)]
struct InstallCommand {
    // Secure Boot Public Key
    #[arg(long)]
    public_key: PathBuf,

    // Secure Boot Private Key
    #[arg(long)]
    private_key: PathBuf,

    // Secure Boot PKI Bundle for auto enrolling key
    #[arg(long)]
    pki_bundle: Option<PathBuf>,

    // Enable auto enrolling your keys in UEFI
    // Be aware that this might irrevocably brick your device
    #[arg(long, default_value = "false")]
    auto_enroll: bool,

    bootspec: PathBuf,

    generations: Vec<PathBuf>,
}

impl Cli {
    pub fn call(self) -> Result<()> {
        self.commands.call()
    }
}

impl Commands {
    pub fn call(self) -> Result<()> {
        match self {
            Commands::Install(args) => install(args),
        }
    }
}

fn install(args: InstallCommand) -> Result<()> {
    let lanzaboote_stub =
        std::env::var("LANZABOOTE_STUB").context("Failed to read LANZABOOTE_STUB env variable")?;
    let initrd_stub = std::env::var("LANZABOOTE_INITRD_STUB")
        .context("Failed to read LANZABOOTE_INITRD_STUB env variable")?;

    install::Installer::new(
        PathBuf::from(lanzaboote_stub),
        PathBuf::from(initrd_stub),
        args.public_key,
        args.private_key,
        args.pki_bundle,
        args.auto_enroll,
        args.bootspec,
        args.generations,
    )
    .install()
}
