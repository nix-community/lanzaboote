use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};

#[derive(Debug)]
pub struct Generation(u64);

impl Generation {
    pub fn from_toplevel(toplevel: impl AsRef<Path>) -> Result<Self> {
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

        let parsed_generation_version = generation_version.parse().with_context(|| {
            format!("Failed to parse generation version: {}", generation_version)
        })?;

        Ok(Self(parsed_generation_version))
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
