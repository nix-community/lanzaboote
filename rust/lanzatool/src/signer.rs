use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::utils;

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

    pub fn sign_and_copy(&self, from: &Path, to: &Path) -> Result<()> {
        let args = vec![
            String::from("--key"),
            utils::path_to_string(&self.private_key),
            String::from("--cert"),
            utils::path_to_string(&self.public_key),
            utils::path_to_string(from),
            String::from("--output"),
            utils::path_to_string(to),
        ];

        let output = Command::new("sbsign").args(&args).output()?;

        if !output.status.success() {
            print!("{:?}", output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to sign file using sbsign with args `{:?}`",
                &args
            )
            .into());
        }

        Ok(())
    }
}
