use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

pub struct KeyPair {
    pub private_key: PathBuf,
    pub public_key: PathBuf,
}

impl KeyPair {
    pub fn new(public_key: &Path, private_key: &Path) -> Self {
        Self {
            public_key: public_key.into(),
            private_key: private_key.into(),
        }
    }

    pub fn sign_and_copy(&self, from: &Path, to: &Path) -> Result<()> {
        let args: Vec<OsString> = vec![
            OsString::from("--key"),
            self.private_key.clone().into(),
            OsString::from("--cert"),
            self.public_key.clone().into(),
            from.as_os_str().to_owned(),
            OsString::from("--output"),
            to.as_os_str().to_owned(),
        ];

        let output = Command::new("sbsign")
            .args(&args)
            .output()
            .context("Failed to run sbsign. Most likely, the binary is not on PATH.")?;

        if !output.status.success() {
            std::io::stderr()
                .write_all(&output.stderr)
                .context("Failed to write output of sbsign to stderr.")?;
            log::debug!("sbsign failed with args: `{args:?}`.");
            return Err(anyhow::anyhow!("Failed to sign {to:?}."));
        }

        Ok(())
    }

    /// Verify the signature of a PE binary. Return true if the signature was verified.
    pub fn verify(&self, path: &Path) -> bool {
        let args: Vec<OsString> = vec![
            OsString::from("--cert"),
            self.public_key.clone().into(),
            path.as_os_str().to_owned(),
        ];

        let output = Command::new("sbverify")
            .args(&args)
            .output()
            .expect("Failed to run sbverify. Most likely, the binary is not on PATH.");

        if !output.status.success() {
            if std::io::stderr().write_all(&output.stderr).is_err() {
                return false;
            };
            log::debug!("sbverify failed with args: `{args:?}`.");
            return false;
        }
        true
    }
}
