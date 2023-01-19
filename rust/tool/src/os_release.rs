use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context, Result};

use crate::generation::Generation;

/// An os-release file represented by a BTreeMap.
///
/// This is implemented using a map, so that it can be easily extended in the future (e.g. by
/// reading the original os-release and patching it).
///
/// The BTreeMap is used over a HashMap, so that the keys are ordered. This is irrelevant for
/// systemd-boot (which does not care about order when reading the os-release file) but is useful
/// for testing. Ordered keys allow using snapshot tests.
pub struct OsRelease(BTreeMap<&'static str, String>);

impl OsRelease {
    pub fn from_generation(generation: &Generation) -> Result<Self> {
        let mut map = BTreeMap::new();

        // Because of a null pointer dereference, `bootctl` segfaults when no ID field is present
        // in the .osrel section of the stub.
        // Fixed in https://github.com/systemd/systemd/pull/25953
        //
        // Because the ID field here does not have the same meaning as in a real os-release file,
        // it is fine to use a dummy value.
        map.insert("ID", String::from("lanza"));
        map.insert("PRETTY_NAME", generation.spec.bootspec.label.clone());
        map.insert(
            "VERSION_ID",
            generation
                .describe()
                .context("Failed to describe generation.")?,
        );

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
