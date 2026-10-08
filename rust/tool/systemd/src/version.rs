use std::fs;
use std::path::Path;
use std::str;
use std::str::FromStr;
use std::{ffi::CStr, fmt};

use anyhow::{Context, Result, bail};

use lanzaboote_tool::os_release::OsRelease;
use lanzaboote_tool::pe;

/// Path to EFI variable that is expected to contain the version string of the booted boot loader
const LOADER_INFO_EFIVAR_PATH: &str =
    "/sys/firmware/efi/efivars/LoaderInfo-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";

/// Length of the EFI variable attributes
///
/// See <https://www.kernel.org/doc/html/latest/filesystems/efivarfs.html>
const EFIVAR_ATTR_LEN: usize = 4;

/// A systemd version.
///
/// systemd does not follow semver standards, but we try to map it anyway. Version components that are not there are treated as zero.
///
/// A notible quirk here is our handling of release candidate
/// versions. We treat 255-rc2 as 255.-1.2, which should give us the
/// correct ordering.
#[derive(PartialEq, PartialOrd, Eq, Debug)]
pub struct SystemdVersion {
    major: u32,

    /// This is a signed integer, so we can model "rc" versions as -1 here.
    minor: i32,

    patch: u32,
}

impl SystemdVersion {
    /// Read the systemd version from the `.osrel` section of a systemd-boot binary.
    pub fn from_systemd_boot_binary(path: &Path) -> Result<Self> {
        let file_data = fs::read(path).with_context(|| format!("Failed to read file {path:?}"))?;
        let section_data = pe::read_section_data(&file_data, ".osrel")
            .with_context(|| format!("PE section '.osrel' is empty: {path:?}"))?;

        // The `.osrel` section in the systemd-boot binary may be NUL-terminated or not
        // so we need to handle both cases.
        let section_data_string = match section_data[section_data.len() - 1] {
            0 => CStr::from_bytes_with_nul(section_data)
                .context("Failed to parse C string.")?
                .to_str()
                .context("Failed to convert C string to Rust string.")?,
            b'\n' => str::from_utf8(section_data)
                .context("Failed to convert section data to Rust string.")?,
            _ => bail!("PE section '.osrel' has unexpected content"),
        };

        let os_release = OsRelease::from_str(section_data_string)
            .with_context(|| format!("Failed to parse os-release from {section_data_string}"))?;

        let version_str = os_release
            .0
            .get("VERSION")
            .context("Failed to extract VERSION key from: {os_release:#?}")?;

        Self::from_str(version_str)
    }

    /// Read the systemd version of the booted boot loader from the `LoaderInfo` EFI variable.
    pub fn from_efivar() -> Result<Self> {
        let data = fs::read(Path::new(LOADER_INFO_EFIVAR_PATH))
            .with_context(|| format!("Failed to read EFI variable {LOADER_INFO_EFIVAR_PATH}"))?;

        Self::from_loader_info(&data)
            .with_context(|| format!("Failed to parse EFI variable {LOADER_INFO_EFIVAR_PATH}"))
    }

    /// Reads and parses `LoaderInfo` EFI variable.
    ///
    /// Set as `systemd-boot <version>` by systemd-boot as NUL-terminated UTF-16 string.
    /// See <https://systemd.io/BOOT_LOADER_INTERFACE/>.
    fn from_loader_info(data: &[u8]) -> Result<Self> {
        let value = data
            .get(EFIVAR_ATTR_LEN..)
            .context("EFI variable is too short to hold expected attribute prefix")?;

        if !value.len().is_multiple_of(2) {
            bail!("Failed to read a whole number of UTF16 code units from EFI variable");
        }

        let code_units: Vec<u16> = value
            .chunks(2)
            .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
            // read until NUL
            .take_while(|unit| *unit != 0)
            .collect();

        let loader_info = String::from_utf16(&code_units)
            .context("Failed to convert UTF16 string to rust string")?;

        let version_str = loader_info.strip_prefix("systemd-boot ").with_context(|| {
            format!("Booted boot loader does not look like systemd-boot: {loader_info}")
        })?;

        Self::from_str(version_str)
    }
}

impl FromStr for SystemdVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((major_str, rc_str)) = s.split_once("-rc") {
            // A version that looks like: 253-rc2
            Ok(Self {
                major: major_str.parse()?,
                minor: -1,
                patch: rc_str.parse()?,
            })
        } else if let Some((major_str, minor_str)) = s.split_once('.') {
            // A version that looks like: 253.7
            Ok(Self {
                major: major_str.parse()?,
                minor: minor_str.parse()?,
                patch: 0,
            })
        } else {
            // A version that looks like: 253
            Ok(Self {
                major: s.parse()?,
                minor: 0,
                patch: 0,
            })
        }
    }
}

/// The inverse of [`SystemdVersion::from_str`].
///
/// The rendered version is used to match boot loader measurements,
/// so it has to round trip with [`SystemdVersion::from_str`].
impl fmt::Display for SystemdVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.major, self.minor, self.patch) {
            (major, minor, rc) if minor < 0 => write!(f, "{major}-rc{rc}"),
            (major, 0, 0) => write!(f, "{major}"),
            (major, minor, 0) => write!(f, "{major}.{minor}"),
            (major, minor, patch) => write!(f, "{major}.{minor}.{patch}"),
        }
    }
}

#[cfg(test)]
impl From<(u32, i32, u32)> for SystemdVersion {
    fn from(value: (u32, i32, u32)) -> Self {
        SystemdVersion {
            major: value.0,
            minor: value.1,
            patch: value.2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_correctly() {
        assert_eq!(parse_version("253"), (253, 0, 0).into());
        assert_eq!(parse_version("252.4"), (252, 4, 0).into());
        assert_eq!(parse_version("251.11"), (251, 11, 0).into());
        assert_eq!(parse_version("251-rc7"), (251, -1, 7).into());
    }

    #[test]
    fn compare_version_correctly() {
        assert!(parse_version("253") > parse_version("252"));
        assert!(parse_version("253") > parse_version("252.4"));
        assert!(parse_version("251.8") == parse_version("251.8"));
        assert!(parse_version("251-rc5") > parse_version("251-rc4"));
        assert!(parse_version("251") > parse_version("251-rc9"));
    }

    #[test]
    fn fail_to_parse_version() {
        parse_version_error("");
        parse_version_error("213;k;13");
        parse_version_error("-1.3.123");
    }

    fn parse_version(input: &str) -> SystemdVersion {
        SystemdVersion::from_str(input).unwrap()
    }

    fn parse_version_error(input: &str) {
        assert!(SystemdVersion::from_str(input).is_err());
    }

    fn render_version(input: impl Into<SystemdVersion>) -> String {
        input.into().to_string()
    }

    #[test]
    fn render_version_correctly() {
        assert_eq!(render_version((253, 0, 0)), "253");
        assert_eq!(render_version((252, 4, 0)), "252.4");
        assert_eq!(render_version((251, 11, 0)), "251.11");
        assert_eq!(render_version((251, -1, 7)), "251-rc7");
    }

    #[test]
    fn version_parse_render_roundtrip() {
        for version in ["262", "258.11", "261-rc1"] {
            assert_eq!(version, parse_version(version).to_string());
        }
    }

    fn loader_info(value: &str) -> Vec<u8> {
        // attribute prefix (see <https://www.kernel.org/doc/html/latest/filesystems/efivarfs.html>)
        let mut data = vec![0x06, 0x00, 0x00, 0x00];

        // UTF16 string
        for unit in value.encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }

        // NUL
        data.extend_from_slice(&0u16.to_le_bytes());
        data
    }

    #[test]
    fn parse_loader_info_correctly() {
        assert_eq!(
            SystemdVersion::from_loader_info(&loader_info("systemd-boot 262")).unwrap(),
            (262, 0, 0).into()
        );
        assert_eq!(
            SystemdVersion::from_loader_info(&loader_info("systemd-boot 258.11")).unwrap(),
            (258, 11, 0).into()
        );
        assert_eq!(
            SystemdVersion::from_loader_info(&loader_info("systemd-boot 261-rc1")).unwrap(),
            (261, -1, 1).into()
        );
    }

    #[test]
    fn fail_to_parse_loader_info() {
        // no version number
        assert!(SystemdVersion::from_loader_info(&loader_info("systemd-boot ")).is_err());
        // something trailing the version number
        assert!(SystemdVersion::from_loader_info(&loader_info("systemd-boot 666 777")).is_err());

        let efivar = loader_info("systemd-boot 262");

        // valid variable data but missing the expected attribute prefix
        assert!(SystemdVersion::from_loader_info(&efivar[EFIVAR_ATTR_LEN..]).is_err());

        // not a whole number of code units
        assert!(SystemdVersion::from_loader_info(&efivar[..efivar.len() - 1]).is_err());

        // no
        assert!(SystemdVersion::from_loader_info(&loader_info("GRUB 2.16")).is_err());
    }
}
