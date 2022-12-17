use serde::{Deserialize, Serialize};

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bootspec::BootJson;
use bootspec::SpecialisationName;
use bootspec::generation::Generation;

// TODO: actually, I'm not sure it's a good thing to have Default
// we should maybe have TryDefault?
// discuss this with upstream.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SecureBootExtension {
    #[serde(rename="org.lanzaboote.osRelease")]
    pub os_release: PathBuf
}

pub type ExtendedBootJson = BootJson<SecureBootExtension>;

#[derive(Debug)]
pub struct OSGeneration {
    /// Top-level nixpkgs version
    version: u64,
    /// Top-level specialisation name
    specialisation_name: Option<SpecialisationName>,
    /// Top-level bootspec document
    pub bootspec: ExtendedBootJson,
}

fn into_boot_json(generation: Generation<SecureBootExtension>) -> Result<ExtendedBootJson> {
    Ok(match generation {
        Generation::V1(json) => json,
        _ => panic!("Failed")
    })
}

impl OSGeneration {
    pub fn from_toplevel(toplevel: impl AsRef<Path>) -> Result<Self> {
        let bootspec_path = toplevel.as_ref().join("bootspec/boot.json");
        let generation: Generation<SecureBootExtension> = serde_json::from_slice(
            &fs::read(bootspec_path).context("Failed to read bootspec file")?,
        )
        .context("Failed to parse bootspec json")?;

        Ok(Self {
            version: parse_version(toplevel)?,
            specialisation_name: None,
            bootspec: into_boot_json(generation)?,
        })
    }

    pub fn specialise(&self, name: &SpecialisationName, bootspec: &ExtendedBootJson) -> Self {
        Self {
            version: self.version,
            specialisation_name: Some(name.clone()),
            bootspec: bootspec.clone()
        }
    }

    pub fn is_specialized(&self) -> Option<SpecialisationName> {
        self.specialisation_name.clone()
    }
}

impl fmt::Display for OSGeneration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

fn parse_version(toplevel: impl AsRef<Path>) -> Result<u64> {
    let file_name = toplevel
        .as_ref()
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Failed to extract file name from generation"))?;

    let file_name_str = file_name
        .to_str()
        .with_context(|| "Failed to convert file name of generation to string")?;

    let generation_version = file_name_str
        .split('-')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Failed to extract version from generation"))?;

    let parsed_generation_version = generation_version
        .parse()
        .with_context(|| format!("Failed to parse generation version: {}", generation_version))?;

    Ok(parsed_generation_version)
}
