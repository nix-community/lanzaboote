use anyhow::Result;

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Signer {
    pub private_key: PathBuf,
    pub public_key: PathBuf,
}

impl Signer {
    pub fn new(public_key: &Path, private_key: &Path) -> Self {
        Self {
            public_key: public_key.into(),
            private_key: private_key.into(),
        }
    }

    pub fn sign_file(&self, filepath: &Path) -> Result<()> {
        let args = vec![
            String::from("--key"),
            String::from(self.private_key.to_str().unwrap()),
            String::from("--cert"),
            String::from(self.public_key.to_str().unwrap()),
            String::from(filepath.to_str().unwrap()),
            String::from("--output"),
            String::from(filepath.to_str().unwrap()),
        ];

        let status = Command::new("sbsign").args(&args).status()?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Failed to sign file using sbsign with args `{:?}`",
                &args
            )
            .into());
        }

        Ok(())
    }
}
