use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use bootspec::BootJson;
use bootspec::BootSpec;
use bootspec::SpecialisationName;
use serde::Deserialize;
use time::Date;

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
/// Partition. The only information missing is the profile and version number which are encoded in
/// the path of the generation link.
#[derive(Debug, Clone)]
pub struct Generation {
    /// Name of the system profile, `None` for the default `system` profile
    pub profile: Option<String>,
    /// Profile symlink index
    pub version: u64,
    /// Build time
    pub build_time: Option<Date>,
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
            version: link.version,
            build_time: link.build_time,
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
            version: link.version,
            build_time: link.build_time,
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

    /// Describe the profile of the generation for humans.
    ///
    /// Emulates how NixOS's current systemd-boot-builder.py adds the profile to the title. Empty
    /// for the default profile.
    pub fn describe_profile(&self) -> String {
        if let Some(profile) = &self.profile {
            format!(" [{profile}]")
        } else {
            "".to_string()
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
            .map(|x| x.to_string())
            .unwrap_or_else(|| String::from("Unknown"));

        format!(
            "Generation {}{}, {}",
            self.version,
            self.describe_specialisation(),
            build_time
        )
    }

    /// A unique short identifier.
    ///
    /// Identifiers of the default profile start with the version number, those of other profiles
    /// with `profile-`, so that the two can never collide.
    pub fn version_tag(&self) -> String {
        if let Some(profile) = &self.profile {
            format!(
                "profile-{}-{}{}",
                profile,
                self.version,
                self.describe_specialisation()
            )
        } else {
            format!("{}{}", self.version, self.describe_specialisation(),)
        }
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
/// Can be built from a symlink in /nix/var/nix/profiles/ alone because the path of the
/// symlink encodes the profile and version number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationLink {
    /// Name of the system profile, `None` for the default `system` profile
    pub profile: Option<String>,
    pub version: u64,
    pub path: PathBuf,
    pub build_time: Option<Date>,
    /// Modification time of the link, to compare generations of different profiles
    pub modified: Option<SystemTime>,
}

impl GenerationLink {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let (profile, version) =
            parse_profile_and_version(&path).context("Failed to parse profile and version")?;
        Ok(Self {
            profile,
            version,
            path: PathBuf::from(path.as_ref()),
            build_time: read_build_time(path.as_ref()).ok(),
            modified: fs::symlink_metadata(path.as_ref())
                .and_then(|m| m.modified())
                .ok(),
        })
    }

    /// Key to order generations of all profiles from oldest to newest.
    pub fn age_key(&self) -> (Option<SystemTime>, &Option<String>, u64) {
        (self.modified, &self.profile, self.version)
    }
}

impl PartialOrd for GenerationLink {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GenerationLink {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.profile, self.version).cmp(&(&other.profile, other.version))
    }
}

/// Parse profile name and version number from a path.
///
/// Expects a path in the format of "system-{version}-link" for the default profile or
/// "system-profiles/{profile}-{version}-link" for other profiles. Profile names may contain "-".
fn parse_profile_and_version(path: impl AsRef<Path>) -> Result<(Option<String>, u64)> {
    let path = path.as_ref();
    let (name, version) = path
        .file_name()
        .and_then(|x| x.to_str())
        .and_then(|x| x.strip_suffix("-link"))
        .and_then(|x| x.rsplit_once('-'))
        .and_then(|(name, version)| Some((name, version.parse::<u64>().ok()?)))
        .with_context(|| format!("Failed to extract version from: {path:?}"))?;

    let in_system_profiles = path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|x| x == "system-profiles");
    let profile = if !in_system_profiles {
        None
    } else if name.is_empty() {
        bail!("Failed to extract profile name from: {path:?}");
    } else if !is_safe_profile_name(name) {
        // The profile name ends up in file names on the ESP, in loader.conf and in the os-release
        // of the stub. Refuse anything that could break those before anything is installed.
        bail!(
            "Profile name {name:?} of {path:?} contains characters other than ASCII letters, digits, '.', '_' and '-'"
        );
    } else {
        Some(name.to_string())
    };

    Ok((profile, version))
}

/// Whether a profile name can be used safely in ESP file names and loader.conf.
fn is_safe_profile_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;

    #[test]
    fn parse_version_correctly() {
        let path = Path::new("system-2-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (None, 2));

        let path = Path::new("/nix/var/nix/profiles/system-12-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (None, 12));
    }

    #[test]
    fn parse_profile_correctly() {
        let path = Path::new("/nix/var/nix/profiles/system-profiles/custom-3-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (Some("custom".to_string()), 3));

        let path = Path::new("system-profiles/my-host-profile-7-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (Some("my-host-profile".to_string()), 7));

        let path = Path::new("system-profiles/system-1-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (Some("system".to_string()), 1));

        let path = Path::new("system-profiles/My.Profile_1-2-link");
        let parsed = parse_profile_and_version(path).unwrap();
        assert_eq!(parsed, (Some("My.Profile_1".to_string()), 2));
    }

    #[test]
    fn reject_unsafe_profile_names() {
        for path in [
            "system-profiles/my profile-1-link",
            "system-profiles/my*profile-1-link",
            "system-profiles/my:profile-1-link",
            "system-profiles/my\nprofile-1-link",
            "system-profiles/mäin-1-link",
        ] {
            let error = parse_profile_and_version(path).unwrap_err();
            assert!(
                error.to_string().contains("contains characters"),
                "{path} should be rejected because of its characters: {error}"
            );
        }
    }

    #[test]
    fn generation_link_ordering() {
        let gen_0 = GenerationLink {
            profile: None,
            version: 0,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };
        let gen_1 = GenerationLink {
            profile: None,
            version: 1,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };
        assert!(gen_1 > gen_0)
    }

    #[test]
    fn generation_link_profile_identity() {
        let system_3 = GenerationLink {
            profile: None,
            version: 3,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };
        let custom_3 = GenerationLink {
            profile: Some("custom".to_string()),
            version: 3,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };
        let custom_1 = GenerationLink {
            profile: Some("custom".to_string()),
            version: 1,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };

        // The default profile sorts first, then other profiles by name and version.
        assert!(system_3 < custom_1);
        assert!(custom_1 < custom_3);

        let set = BTreeSet::from([system_3, custom_3, custom_1]);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn generation_link_set() {
        let gen_0 = GenerationLink {
            profile: None,
            version: 0,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };
        let gen_1 = GenerationLink {
            profile: None,
            version: 1,
            path: PathBuf::new(),
            build_time: None,
            modified: None,
        };

        let mut set = BTreeSet::new();
        set.insert(gen_0);
        set.insert(gen_1);

        let mut iter = set.iter();
        assert_eq!(iter.next().unwrap().version, 0);
        assert_eq!(iter.next().unwrap().version, 1);

        let mut reverse = set.iter().rev();
        assert_eq!(reverse.next().unwrap().version, 1);
        assert_eq!(reverse.next().unwrap().version, 0);
    }
}
