use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bootspec::BootJson;
use bootspec::BootSpec;
use bootspec::SpecialisationName;
use time::Date;

/// (Possibly) extended Bootspec.
///
/// This struct currently does not have any extensions. We keep it around so that extension becomes
/// easy if/when we have to do it.
#[derive(Debug, Clone)]
pub struct ExtendedBootJson {
    pub bootspec: BootSpec,
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
    /// Build time
    build_time: Option<Date>,
    /// Top-level specialisation name
    specialisation_name: Option<SpecialisationName>,
    /// Top-level extended boot specification
    pub spec: ExtendedBootJson,
}

impl Generation {
    pub fn from_link(link: &GenerationLink) -> Result<Self> {
        let bootspec_path = link.path.join("boot.json");
        let boot_json: BootJson = fs::read(bootspec_path)
            .context("Failed to read bootspec file")
            .and_then(|raw| serde_json::from_slice(&raw).context("Failed to read bootspec JSON"))
            .or_else(|_err| BootJson::synthesize_latest(&link.path)
                    .context("Failed to read a bootspec (missing bootspec?) and failed to synthesize a valid replacement bootspec."))?;

        let bootspec: BootSpec = boot_json.generation.try_into()?;

        Ok(Self {
            version: link.version,
            build_time: link.build_time,
            specialisation_name: None,
            spec: ExtendedBootJson { bootspec },
        })
    }

    pub fn specialise(&self, name: &SpecialisationName, bootspec: &BootSpec) -> Result<Self> {
        Ok(Self {
            version: self.version,
            build_time: self.build_time,
            specialisation_name: Some(name.clone()),
            spec: ExtendedBootJson {
                bootspec: bootspec.clone(),
            },
        })
    }

    pub fn is_specialised(&self) -> Option<SpecialisationName> {
        self.specialisation_name.clone()
    }

    /// Describe the generation in a single line.
    ///
    /// Emulates how NixOS's current systemd-boot-builder.py describes generations so that the user
    /// interface remains similar.
    ///
    /// This is currently implemented by poking around the filesystem to find the necessary data.
    /// Ideally, the needed data should be included in the bootspec.
    pub fn describe(&self) -> String {
        let build_time = self
            .build_time
            .map(|x| x.to_string())
            .unwrap_or_else(|| String::from("Unknown"));
        format!("Generation {}, Built on {}", self.version, build_time)
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

fn read_build_time(path: &Path) -> Result<Date> {
    let build_time =
        time::OffsetDateTime::from_unix_timestamp(fs::symlink_metadata(path)?.mtime())?.date();
    Ok(build_time)
}

/// A link pointing to a generation.
///
/// Can be built from a symlink in /nix/var/nix/profiles/ alone because the name of the
/// symlink encodes the version number.
#[derive(Debug)]
pub struct GenerationLink {
    pub version: u64,
    pub path: PathBuf,
    pub build_time: Option<Date>,
}

impl GenerationLink {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            version: parse_version(&path).context("Failed to parse version")?,
            path: PathBuf::from(path.as_ref()),
            build_time: read_build_time(path.as_ref()).ok(),
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
