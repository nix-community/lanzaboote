use std::path::{Path, PathBuf};

use crate::architecture::Architecture;

/// Generic ESP paths which can be specific to a bootloader
pub trait EspPaths<const N: usize> {
    /// Build an ESP path structure out of the ESP root directory
    fn new(esp: impl AsRef<Path>, arch: Architecture) -> Self;

    /// Return the used file paths to store as garbage collection roots.
    fn iter(&self) -> std::array::IntoIter<&PathBuf, N>;

    /// Returns the path containing NixOS EFI binaries
    fn nixos_path(&self) -> &Path;

    /// Returns the path containing Linux EFI binaries
    fn linux_path(&self) -> &Path;
}
