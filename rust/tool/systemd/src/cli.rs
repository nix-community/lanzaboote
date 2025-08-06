use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::install;
use lanzaboote_tool::{architecture::Architecture, signature::local::LocalKeyPair};

/// The default log level.
///
/// 2 corresponds to the level INFO.
const DEFAULT_LOG_LEVEL: usize = 2;

#[derive(Parser)]
pub struct Cli {
    /// Silence all output
    #[arg(short, long)]
    quiet: bool,
    /// Verbose mode (-v, -vv, etc.)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Install(InstallCommand),
}

#[derive(Parser)]
struct InstallCommand {
    /// System for lanzaboote binaries, e.g. defines the EFI fallback path
    #[arg(long)]
    system: String,

    /// Systemd path
    #[arg(long)]
    systemd: PathBuf,

    /// Systemd-boot loader config
    #[arg(long)]
    systemd_boot_loader_config: PathBuf,

    /// sbsign Public Key
    #[arg(long)]
    public_key: Option<PathBuf>,

    /// sbsign Private Key
    #[arg(long)]
    private_key: Option<PathBuf>,

    /// Configuration limit
    #[arg(long, default_value_t = 1)]
    configuration_limit: usize,

    /// A safe generation (usually /run/booted-system) that should always be kept, ignored from --configuration-limit if used
    #[arg(long)]
    safe_generation: Option<PathBuf>,

    /// EFI system partition mountpoint (e.g. efiSysMountPoint)
    esp: PathBuf,

    /// List of generation links (e.g. /nix/var/nix/profiles/system-*-link)
    generations: Vec<PathBuf>,
}

impl Cli {
    pub fn call(self, module: &str) {
        stderrlog::new()
            .module(module)
            .show_level(false)
            .quiet(self.quiet)
            .verbosity(DEFAULT_LOG_LEVEL + usize::from(self.verbose))
            .init()
            .expect("Failed to setup logger.");

        if let Err(e) = self.commands.call() {
            log::error!("{e:#}");
            std::process::exit(1);
        };
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

    let local_signer = LocalKeyPair::new(
        &args.public_key.expect("Failed to obtain public key"),
        &args.private_key.expect("Failed to obtain private key"),
    );

    install::Installer::new(
        PathBuf::from(lanzaboote_stub),
        Architecture::from_nixos_system(&args.system)?,
        args.systemd,
        args.systemd_boot_loader_config,
        local_signer,
        args.configuration_limit,
        args.safe_generation,
        args.esp,
        args.generations,
    )
    .install()
}
