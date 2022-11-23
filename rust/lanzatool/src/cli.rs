use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{crypto, install};

#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate key pair
    Generate,
    /// Sign
    Sign { file: PathBuf, private_key: PathBuf },
    /// Sign
    Verify { file: PathBuf, public_key: PathBuf },
    Install {
        public_key: PathBuf,
        bootspec: PathBuf,
    },
}

impl Cli {
    pub fn call(self) -> Result<()> {
        self.commands.call()
    }
}

impl Commands {
    pub fn call(self) -> Result<()> {
        match self {
            Commands::Generate => generate(),
            Commands::Sign { file, private_key } => sign(&file, &private_key),
            Commands::Verify { file, public_key } => verify(&file, &public_key),
            Commands::Install {
                public_key,
                bootspec,
            } => install(&public_key, &bootspec),
        }
    }
}

fn generate() -> Result<()> {
    let key_pair = crypto::generate_key();

    fs::write("public_key.pem", key_pair.pk.to_pem())?;
    fs::write("private_key.pem", key_pair.sk.to_pem())?;

    Ok(())
}

fn sign(file: &Path, private_key: &Path) -> Result<()> {
    let message = fs::read(file)?;
    let private_key = fs::read_to_string(private_key)?;

    let signature = crypto::sign(&message, &private_key)?;

    let file_path = with_extension(file, ".sig");
    fs::write(file_path, signature.as_slice())?;

    Ok(())
}

fn verify(file: &Path, public_key: &Path) -> Result<()> {
    let message = fs::read(file)?;

    let signature_path = with_extension(file, ".sig");
    let signature = fs::read(signature_path)?;

    let public_key = fs::read_to_string(public_key)?;

    crypto::verify(&message, &signature, &public_key)?;

    Ok(())
}

fn with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut file_path = path.to_path_buf().into_os_string();
    file_path.push(extension);
    PathBuf::from(file_path)
}

fn install(public_key: &Path, bootspec: &Path) -> Result<()> {
    let lanzaboote_bin = std::env::var("LANZABOOTE")?;
    install::install(public_key, bootspec, Path::new(&lanzaboote_bin))
}
