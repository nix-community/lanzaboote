use std::ffi::CStr;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};

use crate::common::os_release::OsRelease;
use crate::common::pe;

/// A systemd version.
///
/// The version is parsed into a u32 tuple because systemd does not follow strict semver
/// conventions. A major version without a minor version, e.g. "252" is represented as `(252, 0)`.
#[derive(PartialEq, PartialOrd, Eq, Debug)]
pub struct SystemdVersion(u32, u32);

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
        let split_version = s
            .split('.')
            .take(2)
            .map(u32::from_str)
            .collect::<Result<Vec<u32>, std::num::ParseIntError>>()
            .context("Failed to parse version string into u32 vector.")?;

        let major = split_version
            .first()
            .context("Failed to parse major version.")?;
        let minor = split_version.get(1).unwrap_or(&0);

        Ok(Self(major.to_owned(), minor.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_correctly() {
        assert_eq!(parse_version("253"), SystemdVersion(253, 0));
        assert_eq!(parse_version("252.4"), SystemdVersion(252, 4));
        assert_eq!(parse_version("251.11"), SystemdVersion(251, 11));
    }

    #[test]
    fn compare_version_correctly() {
        assert!(parse_version("253") > parse_version("252"));
        assert!(parse_version("253") > parse_version("252.4"));
        assert!(parse_version("251.8") == parse_version("251.8"));
    }

    #[test]
    fn fail_to_parse_version() {
        parse_version_error("");
        parse_version_error("213;k;13");
        parse_version_error("-1.3.123");
        parse_version_error("253-rc1");
    }

    fn parse_version(input: &str) -> SystemdVersion {
        SystemdVersion::from_str(input).unwrap()
    }

    fn parse_version_error(input: &str) {
        assert!(SystemdVersion::from_str(input).is_err());
    }
}
