use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

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

        let output = Command::new("sbsign").args(&args).output()?;

        if !output.status.success() {
            std::io::stderr().write_all(&output.stderr).unwrap();
            return Err(anyhow::anyhow!(
                "Failed to sign file using sbsign with args `{:?}`",
                &args
            ));
        }

        Ok(())
    }
}
