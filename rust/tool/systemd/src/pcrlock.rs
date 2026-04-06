use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub struct PcrlockPaths {
    /// The directory containing the Lanzaboote `.pclock` config files for systemd-pcrlock.
    pcrlock: PathBuf,
    lanzaboote: PathBuf,
    bootloader: PathBuf,
}

impl PcrlockPaths {
    pub fn new(pcrlock: impl AsRef<Path>) -> Self {
        let pcrlock = pcrlock.as_ref().to_path_buf();
        Self {
            pcrlock: pcrlock.clone(),
            lanzaboote: pcrlock.join("635-lanzaboote.pcrlock.d"),
            bootloader: pcrlock.join("630-bootloader.pcrlock.d"),
        }
    }

    pub fn lanzaboote(&self) -> &Path {
        &self.lanzaboote
    }

    /// Return the path to a pcrlock measurement file inside the pcrlock directory for Lanzaboote.
    pub fn bootloader_measurement(&self, name: impl AsRef<str>) -> PathBuf {
        self.bootloader.join(format!("{}.pcrlock", name.as_ref()))
    }

    /// Return the path to a pcrlock measurement file inside the pcrlock directory for Lanzaboote.
    pub fn lanzaboote_measurement(&self, name: impl AsRef<str>) -> PathBuf {
        self.lanzaboote.join(format!("{}.pcrlock", name.as_ref()))
    }

    /// Return all pcrlock paths.
    ///
    /// This is useful for including the leading directories in the GC roots.
    pub fn iter(&self) -> std::array::IntoIter<&PathBuf, 2> {
        [&self.pcrlock, &self.lanzaboote].into_iter()
    }
}

/// Lock a PE binary with systemd-pcrlock and write the pcrlock component.
///
/// This calls `systemd-pcrlock lock-pe` and writes the component to `pcrlock_component`.
pub fn lock_pe(binary_path: impl AsRef<Path>, pcrlock_component: impl AsRef<Path>) -> Result<()> {
    let status = Command::new("systemd-pcrlock")
        .arg("lock-pe")
        .arg(binary_path.as_ref())
        .arg("--pcrlock")
        .arg(pcrlock_component.as_ref())
        .status()
        .context("Failed to run systemd-pcrlock. Most likely, the binary is not on PATH")?;
    if !status.success() {
        bail!(
            "Failed to lock PE binary {} via systemd-pcrlock and write pcrlock component to {}",
            binary_path.as_ref().display(),
            pcrlock_component.as_ref().display()
        );
    }

    Ok(())
}
