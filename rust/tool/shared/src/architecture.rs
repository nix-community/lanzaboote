use std::path::PathBuf;

use anyhow::{bail, Result};

/// Supported system
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Architecture {
    X86,
    AArch64,
}

impl Architecture {
    pub fn efi_representation(&self) -> &str {
        match self {
            Self::X86 => "x64",
            Self::AArch64 => "aa64",
        }
    }

    pub fn efi_fallback_filename(&self) -> PathBuf {
        format!("BOOT{}.EFI", self.efi_representation().to_ascii_uppercase()).into()
    }
}

impl Architecture {
    /// Converts from a NixOS system double to a supported system
    pub fn from_nixos_system(system_double: &str) -> Result<Self> {
        Ok(match system_double {
            "x86_64-linux" => Self::X86,
            "aarch64-linux" => Self::AArch64,
            _ => bail!(format!("Unsupported NixOS system: {}.", system_double)),
        })
    }
}
