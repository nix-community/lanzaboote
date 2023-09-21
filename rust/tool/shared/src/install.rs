use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result};
use crate::builder::{GenerationArtifacts, FileSource};
use crate::signature::KeyPair;
use crate::utils::file_hash;

impl GenerationArtifacts {
    /// Install all files to the ESP.
    pub fn install(&self, key_pair: &KeyPair) -> Result<()> {
        for (to, from) in &self.files {
            match from {
                FileSource::SignedFile(from) => {
                    install_signed(key_pair, from, to).with_context(|| {
                        format!("Failed to sign and install from {from:?} to {to:?}")
                    })?
                }
                FileSource::UnsignedFile(from) => install(from, to)
                    .with_context(|| format!("Failed to install from {from:?} to {to:?}"))?,
            }
        }

        Ok(())
    }
}

/// Install a PE file. The PE gets signed in the process.
///
/// If the file already exists at the destination, it is overwritten.
///
/// This is implemented as an atomic write. The file is first written to the destination with a
/// `.tmp` suffix and then renamed to its final name. This is atomic, because a rename is an atomic
/// operation on POSIX platforms.
pub fn install_signed(key_pair: &KeyPair, from: &Path, to: &Path) -> Result<()> {
    log::debug!("Signing and installing {to:?}...");
    let to_tmp = to.with_extension(".tmp");
    ensure_parent_dir(&to_tmp);
    key_pair
        .sign_and_copy(from, &to_tmp)
        .with_context(|| format!("Failed to copy and sign file from {from:?} to {to:?}"))?;
    fs::rename(&to_tmp, to).with_context(|| {
        format!("Failed to move temporary file {to_tmp:?} to final location {to:?}")
    })?;
    Ok(())
}

/// Install an arbitrary file.
///
/// The file is only copied if
///     (1) it doesn't exist at the destination or,
///     (2) the hash of the file at the destination does not match the hash of the source file.
pub fn install(from: &Path, to: &Path) -> Result<()> {
    if !to.exists() || file_hash(from)? != file_hash(to)? {
        force_install(from, to)?;
    }
    Ok(())
}

/// Forcibly install an arbitrary file.
///
/// If the file already exists at the destination, it is overwritten.
///
/// This function is only designed to copy files to the ESP. It sets the permission bits of the
/// file at the destination to 0o755, the expected permissions for a vfat ESP. This is useful for
/// producing file systems trees which can then be converted to a file system image.
pub fn force_install(from: &Path, to: &Path) -> Result<()> {
    log::debug!("Installing {to:?}...");
    ensure_parent_dir(to);
    atomic_copy(from, to)?;
    set_permission_bits(to, 0o755)
        .with_context(|| format!("Failed to set permission bits to 0o755 on file: {to:?}"))?;
    Ok(())
}

/// Atomically copy a file.
///
/// The file is first written to the destination with a `.tmp` suffix and then renamed to its final
/// name. This is atomic, because a rename is an atomic operation on POSIX platforms.
pub fn atomic_copy(from: &Path, to: &Path) -> Result<()> {
    let to_tmp = to.with_extension(".tmp");

    fs::copy(from, &to_tmp)
        .with_context(|| format!("Failed to copy from {from:?} to {to_tmp:?}",))?;

    fs::rename(&to_tmp, to).with_context(|| {
        format!("Failed to move temporary file {to_tmp:?} to final location {to:?}")
    })
}

/// Set the octal permission bits of the specified file.
pub fn set_permission_bits(path: &Path, permission_bits: u32) -> Result<()> {
    let mut perms = fs::metadata(path)
        .with_context(|| format!("File {path:?} doesn't have any metadata"))?
        .permissions();
    perms.set_mode(permission_bits);
    fs::set_permissions(path, perms)
        .with_context(|| format!("Failed to set permissions on {path:?}"))
}

// Ensures the parent directory of an arbitrary path exists
pub fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
}

