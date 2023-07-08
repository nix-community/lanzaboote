use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context, Result};

use crate::common::generation::Generation;

/// An os-release file represented by a BTreeMap.
///
/// This is implemented using a map, so that it can be easily extended in the future (e.g. by
/// reading the original os-release and patching it).
///
/// The BTreeMap is used over a HashMap, so that the keys are ordered. This is irrelevant for
/// systemd-boot (which does not care about order when reading the os-release file) but is useful
/// for testing. Ordered keys allow using snapshot tests.
pub struct OsRelease(pub BTreeMap<String, String>);

impl OsRelease {
    pub fn from_generation(generation: &Generation) -> Result<Self> {
        let mut map = BTreeMap::new();

        // Because of a null pointer dereference, `bootctl` segfaults when no ID field is present
        // in the .osrel section of the stub.
        // Fixed in https://github.com/systemd/systemd/pull/25953
        //
        // Because the ID field here does not have the same meaning as in a real os-release file,
        // it is fine to use a dummy value.
        map.insert("ID".into(), String::from("lanza"));
        map.insert(
            "PRETTY_NAME".into(),
            generation.spec.bootspec.bootspec.label.clone(),
        );
        map.insert("VERSION_ID".into(), generation.describe());

        Ok(Self(map))
    }

    /// Parse the string representation of a os-release file.
    ///
    /// **Beware before reusing this function!**
    ///
    /// This parser might not parse all valid os-release files correctly. It is only designed to
    /// read the `VERSION` key from the os-release of a systemd-boot binary.
    pub fn from_str(value: &str) -> Result<Self> {
        let mut map = BTreeMap::new();

        // All valid lines
        let lines = value
            .lines()
            .map(str::trim)
            .filter(|x| !x.starts_with('#') && !x.is_empty());
        // Split into keys/values
        let key_value_lines = lines.map(|x| x.split('=').collect::<Vec<&str>>());
        for kv in key_value_lines {
            let k = kv
                .first()
                .with_context(|| format!("Failed to get first element from {kv:?}"))?;
            let v = kv
                .get(1)
                .map(|s| s.strip_prefix(|c| c == '"' || c == '\'').unwrap_or(s))
                .map(|s| s.strip_suffix(|c| c == '"' || c == '\'').unwrap_or(s))
                .with_context(|| format!("Failed to get second element from {kv:?}"))?;
            // Clean up the value. We already have the value without leading/tailing "
            // so we just need to unescape the string.
            let v = v
                .replace("\\$", "$")
                .replace("\\\"", "\"")
                .replace("\\`", "`")
                .replace("\\\\", "\\");

            map.insert(String::from(*k), v);
        }

        Ok(Self(map))
    }
}

/// Display OsRelease in the format of an os-release file.
impl fmt::Display for OsRelease {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (key, value) in &self.0 {
            writeln!(f, "{}={}", key, value)?
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn parses_correctly_from_str() -> Result<()> {
        let os_release_cstr = CStr::from_bytes_with_nul(b"ID=systemd-boot\nVERSION=\"252.1\"\n\0")?;
        let os_release_str = os_release_cstr.to_str()?;
        let os_release = OsRelease::from_str(os_release_str)?;

        assert!(os_release.0["ID"] == "systemd-boot");
        assert!(os_release.0["VERSION"] == "252.1");

        Ok(())
    }

    #[test]
    fn escaping_works() -> Result<()> {
        let teststring = r#"
            NO_QUOTES=systemd-boot
            DOUBLE_QUOTES="systemd-boot"
            SINGLE_QUOTES='systemd-boot'
            UNESCAPED_DOLLAR=$1.2
            ESCAPED_DOLLAR=\$1.2
            UNESCAPED_BACKTICK=`1.2
            ESCAPED_BACKTICK=\`1.2
            UNESCAPED_QUOTE=""1.2"
            ESCAPED_QUOTE=\"1.2
        "#;
        let os_release = OsRelease::from_str(teststring)?;

        assert!(os_release.0["NO_QUOTES"] == "systemd-boot");
        assert!(os_release.0["DOUBLE_QUOTES"] == "systemd-boot");
        assert!(os_release.0["SINGLE_QUOTES"] == "systemd-boot");
        assert!(os_release.0["UNESCAPED_DOLLAR"] == "$1.2");
        assert!(os_release.0["ESCAPED_DOLLAR"] == "$1.2");
        assert!(os_release.0["UNESCAPED_BACKTICK"] == "`1.2");
        assert!(os_release.0["ESCAPED_BACKTICK"] == "`1.2");
        assert!(os_release.0["UNESCAPED_QUOTE"] == "\"1.2");
        assert!(os_release.0["ESCAPED_QUOTE"] == "\"1.2");

        Ok(())
    }
}
