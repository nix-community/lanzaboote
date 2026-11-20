use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use lanzaboote_tool::generation::GenlistEntry;

/// Read a genlist.json file and return a vector of GenlistEntry.
/// It also validates the file format.
pub fn read(genlist_path: impl AsRef<Path>) -> Result<Vec<GenlistEntry>> {
    use std::collections::BTreeSet;

    let file = File::open(genlist_path.as_ref()).context("Failed to open generations list file")?;
    let reader = BufReader::new(file);

    let mut entries = Vec::new();
    let mut seen_versions = BTreeSet::new();

    for line in reader.lines() {
        let line = line.context("Failed to read a line from the generations list file")?;
        let entry: GenlistEntry = serde_json::from_str(&line)
            .context("Failed to parse a generation from the generations list file")?;

        // Check for duplicate versions
        if !seen_versions.insert(entry.version) {
            anyhow::bail!("Genlist contains duplicate version: {}", entry.version);
        }

        // Validate build_time is a valid Unix timestamp if present (same as in shared/src/generation.rs:107-108)
        if let Some(build_time) = entry.build_time {
            let _ = time::OffsetDateTime::from_unix_timestamp(build_time)
                .context("Failed to parse the generation build time")?;
        }

        // Check that label doesn't contain newline
        if entry.label.contains('\n') {
            anyhow::bail!(
                "Genlist entry label contains newline character: {:?}",
                entry.label
            );
        }

        entries.push(entry);
    }

    // Verify entries are ordered from smallest to highest version number
    for i in 1..entries.len() {
        if entries[i - 1].version >= entries[i].version {
            anyhow::bail!(
                "Genlist entries are not ordered from smallest to highest version. \
                 Entry at index {} has version {} but entry at index {} has version {}",
                i - 1,
                entries[i - 1].version,
                i,
                entries[i].version
            );
        }
    }

    Ok(entries)
}
