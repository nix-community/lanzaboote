use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use bootspec::generation::Generation as BootspecGeneration;
use bootspec::BootJson;
use bootspec::SpecialisationName;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureBootExtension {
    #[serde(rename = "osRelease")]
    pub os_release: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ExtendedBootJson {
    pub bootspec: BootJson,
    pub extensions: SecureBootExtension,
}

#[derive(Debug)]
pub struct Generation {
    /// Profile symlink index
    version: u64,
    /// Top-level specialisation name
    specialisation_name: Option<SpecialisationName>,
    /// Top-level extended boot specification
    pub spec: ExtendedBootJson,
}

impl Generation {
    fn extract_extensions(bootspec: &BootJson) -> Result<SecureBootExtension> {
        Ok(Deserialize::deserialize(
            bootspec.extensions.get("lanzaboote")
            .context("Failed to extract Lanzaboote-specific extension from Bootspec, missing lanzaboote field in `extensions`")?
            .clone()
            .into_deserializer()
        )?)
    }

    pub fn from_toplevel(toplevel: impl AsRef<Path>) -> Result<Self> {
        let bootspec_path = toplevel.as_ref().join("boot.json");
        let generation: BootspecGeneration = serde_json::from_slice(
            &fs::read(bootspec_path).context("Failed to read bootspec file")?,
        )
        .context("Failed to parse bootspec json")?;

        let bootspec: BootJson = generation
            .try_into()
            .map_err(|err: &'static str| anyhow!(err))?;

        let extensions = Self::extract_extensions(&bootspec)?;

        Ok(Self {
            version: parse_version(toplevel)?,
            specialisation_name: None,
            spec: ExtendedBootJson {
                bootspec,
                extensions,
            },
        })
    }

    pub fn specialise(&self, name: &SpecialisationName, bootspec: &BootJson) -> Result<Self> {
        Ok(Self {
            version: self.version,
            specialisation_name: Some(name.clone()),
            spec: ExtendedBootJson {
                bootspec: bootspec.clone(),
                extensions: Self::extract_extensions(bootspec)?,
            },
        })
    }

    pub fn is_specialized(&self) -> Option<SpecialisationName> {
        self.specialisation_name.clone()
    }
}

impl fmt::Display for Generation {
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
