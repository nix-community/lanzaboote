use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

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

    pub fn bootloader(&self) -> &Path {
        &self.bootloader
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
    pub fn iter(&self) -> std::array::IntoIter<&PathBuf, 3> {
        [&self.pcrlock, &self.lanzaboote, &self.bootloader].into_iter()
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

/// Lock a PE binary with systemd-pcrlock and write the pcrlock component.
///
/// Same as [`lock_pe`], except that the PE binary is fed to `systemd-pcrlock lock-pe` on stdin
/// instead of being read by it from a path.
pub fn lock_pe_from_bytes(
    data: impl AsRef<[u8]>,
    pcrlock_component: impl AsRef<Path>,
) -> Result<()> {
    let mut child = Command::new("systemd-pcrlock")
        .arg("lock-pe")
        .arg("--pcrlock")
        .arg(pcrlock_component.as_ref())
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to run systemd-pcrlock. Most likely, the binary is not on PATH")?;

    let written = child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(data.as_ref());

    let status = child
        .wait()
        .context("Failed to wait for systemd-pcrlock to exit")?;

    if !status.success() {
        bail!(
            "Failed to lock PE from stdin via systemd-pcrlock and write pcrlock component to {}",
            pcrlock_component.as_ref().display()
        );
    }
    written.context("Failed to write to stdin of systemd-pcrlock")?;

    Ok(())
}
