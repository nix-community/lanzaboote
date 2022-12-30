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

/// Parse version number from a path.
///
/// Expects a path in the format of "system-{version}-link".
fn parse_version(path: impl AsRef<Path>) -> Result<u64> {
    let generation_version = path
        .as_ref()
        .file_name()
        .and_then(|x| x.to_str())
        .and_then(|x| x.split('-').nth(1))
        .and_then(|x| x.parse::<u64>().ok())
        .with_context(|| format!("Failed to extract version from: {:?}", path.as_ref()))?;

    Ok(generation_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_correctly() {
        let path = Path::new("system-2-link");
        let parsed_version = parse_version(path).unwrap();
        assert_eq!(parsed_version, 2,);
    }
}
