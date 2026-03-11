use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bootspec::BootJson;
use bootspec::BootSpec;
use bootspec::SpecialisationName;
use serde::Deserialize;
use time::OffsetDateTime;

pub type GenerationTime = OffsetDateTime;

/// (Possibly) extended Bootspec.
///
/// This struct currently does not have any extensions. We keep it around so that extension becomes
/// easy if/when we have to do it.
#[derive(Debug, Clone)]
pub struct ExtendedBootJson {
    pub bootspec: BootSpec,
    pub lanzaboote_extension: LanzabooteExtension,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LanzabooteExtension {
    pub sort_key: String,
}

impl Default for LanzabooteExtension {
    fn default() -> Self {
        Self {
            sort_key: String::from("lanzaboote"),
        }
    }
}

impl From<bootspec::Specialisation> for LanzabooteExtension {
    fn from(spec: bootspec::Specialisation) -> Self {
        spec.extensions
            .get("org.nix-community.lanzaboote")
            .and_then(|v| serde_json::from_value::<LanzabooteExtension>(v.clone()).ok())
            .unwrap_or_default()
    }
}

impl From<bootspec::BootJson> for LanzabooteExtension {
    fn from(spec: bootspec::BootJson) -> Self {
        spec.extensions
            .get("org.nix-community.lanzaboote")
            .and_then(|v| serde_json::from_value::<LanzabooteExtension>(v.clone()).ok())
            .unwrap_or_default()
    }
}

/// A system configuration.
///
/// Can be built from a GenerationLink.
///
/// NixOS represents a generation as a symlink to a toplevel derivation. This toplevel derivation
/// contains most of the information necessary to install the generation onto the EFI System
/// Partition. The only information missing is the version number which is encoded in the file name
/// of the generation link.
#[derive(Debug, Clone)]
pub struct Generation {
    /// Profile symlink name
    pub profile: String,
    /// Whether this generation comes from the default /nix/var/nix/profiles/system-*-link path.
    pub is_default_profile: bool,
    /// Profile symlink index
    pub version: u64,
    /// Generation build time derived from the underlying generation contents, not the profile link.
    pub build_time: Option<GenerationTime>,
    /// Top-level specialisation name
    pub specialisation_name: Option<SpecialisationName>,
    /// Top-level extended boot specification
    pub spec: ExtendedBootJson,
    /// The set of specialisations of this generation
    pub specialisations: HashMap<SpecialisationName, Generation>,
}

impl Generation {
    pub fn from_link(link: &GenerationLink) -> Result<Self> {
        let bootspec_path = link.path.join("boot.json");
        let boot_json: BootJson = fs::read(bootspec_path)
            .context("Failed to read bootspec file")
            .and_then(|raw| serde_json::from_slice(&raw).context("Failed to read bootspec JSON"))
            .or_else(|_err| BootJson::synthesize_latest(&link.path)
                    .context("Failed to read a bootspec (missing bootspec?) and failed to synthesize a valid replacement bootspec."))?;

        Self::parse_boot_json(link, None, boot_json)
    }

    fn parse_specialisation(
        link: &GenerationLink,
        specialisation_name: SpecialisationName,
        specialisation: bootspec::Specialisation,
    ) -> Result<Self> {
        Ok(Self {
            profile: link.profile.clone(),
            is_default_profile: link.is_default_profile,
            version: link.version,
            build_time: link.link_time,
            specialisation_name: Some(specialisation_name),
            spec: ExtendedBootJson {
                bootspec: specialisation.clone().generation,
                lanzaboote_extension: specialisation.clone().into(),
            },
            specialisations: Self::parse_specialisations(
                link,
                specialisation.generation.specialisations,
            )?,
        })
    }

    fn parse_specialisations(
        link: &GenerationLink,
        specialisations: bootspec::Specialisations,
    ) -> Result<HashMap<SpecialisationName, Generation>> {
        specialisations
            .into_iter()
            .map(|(name, json)| {
                Self::parse_specialisation(link, name.clone(), json)
                    .map(|generation| (name, generation))
            })
            .collect::<Result<HashMap<SpecialisationName, Generation>>>()
    }

    fn parse_boot_json(
        link: &GenerationLink,
        specialisation_name: Option<SpecialisationName>,
        boot_json: BootJson,
    ) -> Result<Self> {
        let bootspec: BootSpec = boot_json.clone().generation.try_into()?;

        Ok(Self {
            profile: link.profile.clone(),
            is_default_profile: link.is_default_profile,
            version: link.version,
            build_time: link.link_time,
            specialisation_name,
            spec: ExtendedBootJson {
                bootspec: bootspec.clone(),
                lanzaboote_extension: boot_json.into(),
            },
            specialisations: Self::parse_specialisations(link, bootspec.specialisations)?,
        })
    }

    /// A helper for describe functions below.
    fn describe_specialisation(&self) -> String {
        if let Some(specialization) = &self.specialisation_name {
            format!("-{specialization}")
        } else {
            "".to_string()
        }
    }

    /// Describe the generation profile name for humans.
    pub fn describe_profile(&self) -> String {
        if self.is_default_profile {
            String::new()
        } else {
            format!(" [{}]", self.profile)
        }
    }

    /// Describe the generation in a single line for humans.
    ///
    /// Emulates how NixOS's current systemd-boot-builder.py describes generations so that the user
    /// interface remains similar.
    ///
    /// This is currently implemented by poking around the filesystem to find the necessary data.
    /// Ideally, the needed data should be included in the bootspec.
    pub fn describe(&self) -> String {
        let build_time = self
            .build_time
            .map(|x| x.date().to_string())
            .unwrap_or_else(|| String::from("Unknown"));

        format!(
            "Generation {}{}, {}",
            self.version,
            self.describe_specialisation(),
            build_time
        )
    }

    /// A unique short identifier.
    pub fn version_tag(&self) -> String {
        format!("{}{}", self.version, self.describe_specialisation(),)
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

fn read_build_time(path: &Path) -> Result<GenerationTime> {
    let metadata = fs::symlink_metadata(path)?;
    let build_time = OffsetDateTime::from_unix_timestamp_nanos(
        i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec()),
    )?;
    Ok(build_time)
}

/// A link pointing to a generation.
///
/// Can be built from a symlink in /nix/var/nix/profiles/ alone because the name of the
/// symlink encodes the version number.
#[derive(Debug)]
pub struct GenerationLink {
    pub profile: String,
    pub is_default_profile: bool,
    pub version: u64,
    pub path: PathBuf,
    pub link_time: Option<GenerationTime>,
}

impl GenerationLink {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            profile: parse_profile(&path).context("Failed to parse profile name")?,
            is_default_profile: parse_is_default_profile(&path),
            version: parse_version(&path).context("Failed to parse version")?,
            path: PathBuf::from(path.as_ref()),
            link_time: read_build_time(path.as_ref()).ok(),
        })
    }
}

fn parse_is_default_profile(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        != Some("system-profiles")
}

/// Parse profile name from a path.
///
/// Expects a path in the format of "{profile}-{version}-link".
fn parse_profile(path: impl AsRef<Path>) -> Result<String> {
    let file_name = path
        .as_ref()
        .file_name()
        .and_then(|x| x.to_str())
        .with_context(|| format!("Failed to extract file name from: {:?}", path.as_ref()))?;

    let profile_and_version = file_name
        .strip_suffix("-link")
        .with_context(|| format!("Generation link is missing '-link' suffix: {file_name:?}"))?;

    let profile = profile_and_version
        .rsplit_once('-')
        .map(|(profile, _)| profile)
        .filter(|profile| !profile.is_empty())
        .with_context(|| format!("Failed to extract profile name from: {:?}", path.as_ref()))?;

    Ok(profile.to_string())
}

/// Parse version number from a path.
///
/// Expects a path in the format of "{profile}-{version}-link".
fn parse_version(path: impl AsRef<Path>) -> Result<u64> {
    let file_name = path
        .as_ref()
        .file_name()
        .and_then(|x| x.to_str())
        .with_context(|| format!("Failed to extract file name from: {:?}", path.as_ref()))?;

    let profile_and_version = file_name
        .strip_suffix("-link")
        .with_context(|| format!("Generation link is missing '-link' suffix: {file_name:?}"))?;

    let generation_version = profile_and_version
        .rsplit_once('-')
        .map(|(_, version)| version)
        .and_then(|x| x.parse::<u64>().ok())
        .with_context(|| format!("Failed to extract version from: {:?}", path.as_ref()))?;

    Ok(generation_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_profile_correctly() {
        let path = Path::new("system-2-link");
        let parsed_profile = parse_profile(path).unwrap();
        assert_eq!(parsed_profile, "system");

        let path = Path::new("my-2nd-nixos-machine-3-link");
        let parsed_profile = parse_profile(path).unwrap();
        assert_eq!(parsed_profile, "my-2nd-nixos-machine");
    }

    #[test]
    fn parse_version_correctly() {
        let path = Path::new("system-2-link");
        let parsed_version = parse_version(path).unwrap();
        assert_eq!(parsed_version, 2,);
    }

    #[test]
    fn reject_malformed_generation_link_names() {
        for name in ["system-link", "system-abc-link", "2-link", "system-2"] {
            let path = Path::new(name);
            assert!(parse_profile(path).is_err() || parse_version(path).is_err());
        }
    }
}
