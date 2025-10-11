use std::path::PathBuf;

use lanzaboote_tool::architecture::Architecture;

/// rEFInd-specific architecture helpers
pub trait RefindArchitectureExt {
    fn refind_filename(&self) -> PathBuf;
}

impl RefindArchitectureExt for Architecture {
    fn refind_filename(&self) -> PathBuf {
        format!("refind_{}.efi", self.efi_representation()).into()
    }
}
