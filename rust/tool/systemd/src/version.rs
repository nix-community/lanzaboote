use std::ffi::CStr;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};

use lanzaboote_tool::os_release::OsRelease;
use lanzaboote_tool::pe;

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

        // The `.osrel` section in the systemd-boot binary is a NUL terminated string and thus needs
        // special handling.
        let section_data_cstr =
            CStr::from_bytes_with_nul(section_data).context("Failed to parse C string.")?;
        let section_data_string = section_data_cstr
            .to_str()
            .context("Failed to convert C string to Rust string.")?;

        let os_release = OsRelease::from_str(section_data_string)
            .with_context(|| format!("Failed to parse os-release from {section_data_string}"))?;

        let version_str = os_release
            .0
            .get("VERSION")
            .context("Failed to extract VERSION key from: {os_release:#?}")?;

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
}
