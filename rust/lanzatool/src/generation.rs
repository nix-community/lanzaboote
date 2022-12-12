use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use bootspec::generation::Generation as BootspecGeneration;
use bootspec::BootJson;
use bootspec::SpecialisationName;
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

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

/// A system configuration.
///
/// Can be built from a GenerationLink.
///
/// NixOS represents a generation as a symlink to a toplevel derivation. This toplevel derivation
/// contains most of the information necessary to install the generation onto the EFI System
/// Partition. The only information missing is the version number which is encoded in the file name
/// of the generation link.
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
    pub fn from_link(link: &GenerationLink) -> Result<Self> {
        let bootspec_path = link.path.join("boot.json");
        let generation: BootspecGeneration = serde_json::from_slice(
            &fs::read(bootspec_path).context("Failed to read bootspec file")?,
        )
        .context("Failed to parse bootspec json")?;

        let bootspec: BootJson = generation
            .try_into()
            .map_err(|err: &'static str| anyhow!(err))?;

        let extensions = Self::extract_extensions(&bootspec)?;

        Ok(Self {
            version: link.version,
            specialisation_name: None,
            spec: ExtendedBootJson {
                bootspec,
                extensions,
            },
        })
    }

    fn extract_extensions(bootspec: &BootJson) -> Result<SecureBootExtension> {
        Ok(Deserialize::deserialize(
            bootspec.extensions.get("lanzaboote")
            .context("Failed to extract Lanzaboote-specific extension from Bootspec, missing lanzaboote field in `extensions`")?
            .clone()
            .into_deserializer()
        )?)
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

/// A link pointing to a generation.
///
/// Can be built from a symlink in /nix/var/nix/profiles/ alone because the name of the
/// symlink enocdes the version number.
#[derive(Debug)]
pub struct GenerationLink {
    pub version: u64,
    pub path: PathBuf,
}

impl GenerationLink {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            version: parse_version(&path).context("Failed to parse version")?,
            path: PathBuf::from(path.as_ref()),
        })
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
