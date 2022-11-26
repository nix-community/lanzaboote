use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::bootspec::Bootspec;

#[derive(Debug)]
pub struct Generation {
    version: u64,
    pub bootspec: Bootspec,
}

impl Generation {
    pub fn from_toplevel(toplevel: impl AsRef<Path>) -> Result<Self> {
        let bootspec_path = toplevel.as_ref().join("bootspec/boot.v1.json");
        let bootspec: Bootspec = serde_json::from_slice(
            &fs::read(&bootspec_path).context("Failed to read bootspec file")?,
        )
        .context("Failed to parse bootspec json")?;

        Ok(Self {
            version: parse_version(toplevel)?,
            bootspec,
        })
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

fn parse_version(toplevel: impl AsRef<Path>) -> Result<u64> {
    let file_name = toplevel.as_ref().file_name().ok_or(anyhow::anyhow!(
        "Failed to extract file name from generation"
    ))?;

    let file_name_str = file_name
        .to_str()
        .with_context(|| "Failed to convert file name of generation to string")?;

    let generation_version = file_name_str
        .split("-")
        .nth(1)
        .ok_or(anyhow::anyhow!("Failed to extract version from generation"))?;

    let parsed_generation_version = generation_version
        .parse()
        .with_context(|| format!("Failed to parse generation version: {}", generation_version))?;

    Ok(parsed_generation_version)
}
