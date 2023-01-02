use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use bootspec::generation::Generation as BootspecGeneration;
use bootspec::BootJson;
use bootspec::SpecialisationName;

/// (Possibly) extended Bootspec.
///
/// This struct currently does not have any extensions. We keep it around so that extension becomes
/// easy if/when we have to do it.
#[derive(Debug, Clone)]
pub struct ExtendedBootJson {
    pub bootspec: BootJson,
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

        Ok(Self {
            version: link.version,
            specialisation_name: None,
            spec: ExtendedBootJson { bootspec },
        })
    }

    pub fn specialise(&self, name: &SpecialisationName, bootspec: &BootJson) -> Result<Self> {
        Ok(Self {
            version: self.version,
            specialisation_name: Some(name.clone()),
            spec: ExtendedBootJson {
                bootspec: bootspec.clone(),
            },
        })
    }

    pub fn is_specialized(&self) -> Option<SpecialisationName> {
        self.specialisation_name.clone()
    }

    /// Describe the generation in a single line.
    ///
    /// Emulates how NixOS's current systemd-boot-builder.py describes generations so that the user
    /// interface remains similar.
    ///
    /// This is currently implemented by poking around the filesystem to find the necessary data.
    /// Ideally, the needed data should be included in the bootspec.
    pub fn describe(&self) -> Result<String> {
        let toplevel = &self.spec.bootspec.toplevel.0;

        let nixos_version = fs::read_to_string(toplevel.join("nixos-version"))
            .unwrap_or_else(|_| String::from("Unknown"));
        let kernel_version =
            read_kernel_version(toplevel).context("Failed to read kernel version.")?;
        let build_time = read_build_time(toplevel).unwrap_or_else(|_| String::from("Unknown"));

        Ok(format!(
            "Generation {} NixOS {}, Linux Kernel {}, Built on {}",
            self.version, nixos_version, kernel_version, build_time
        ))
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

/// Read the kernel version from the name of a directory inside the toplevel directory.
///
/// The path looks something like this: $toplevel/kernel-modules/lib/modules/6.1.1
fn read_kernel_version(toplevel: &Path) -> Result<String> {
    let path = fs::read_dir(toplevel.join("kernel-modules/lib/modules"))?
        .into_iter()
        .next()
        .transpose()?
        .map(|x| x.path())
        .with_context(|| format!("Failed to read directory {:?}.", toplevel))?;

    let file_name = path
        .file_name()
        .and_then(|x| x.to_str())
        .context("Failed to convert path to filename string.")?;

    Ok(String::from(file_name))
}

fn read_build_time(path: &Path) -> Result<String> {
    let build_time = time::OffsetDateTime::from_unix_timestamp(fs::metadata(path)?.mtime())?
        .date()
        .to_string();
    Ok(build_time)
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
