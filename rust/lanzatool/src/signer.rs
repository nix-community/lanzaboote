use anyhow::Result;

use std::process::Command;
use std::path::{Path, PathBuf};

pub struct Signer<'a> {
    pub sbsigntool: PathBuf,
    pub private_key: &'a Path,
    pub public_key: &'a Path
}

impl<'a> Signer<'a> {
    pub fn new(signer: &Path, public_key: &'a Path, private_key: &'a Path) -> Self {
        Self {
            sbsigntool: signer.to_path_buf(),
            public_key: public_key,
            private_key: private_key
        }
    }

    pub fn sign_file(&self, filepath: &Path) -> Result<()> {
        let args = vec![
            String::from("--key"),
            String::from(self.private_key.to_str().unwrap()),
            String::from("--cert"),
            String::from(self.public_key.to_str().unwrap()),
            String::from(filepath.to_str().unwrap())
        ];

        let status = Command::new(&self.sbsigntool)
            .args(&args)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                    "Failed success run `{}` with args `{:?}`",
                    &self.sbsigntool.display(),
                    &args
            ).into());
        }

        Ok(())
    }
}
