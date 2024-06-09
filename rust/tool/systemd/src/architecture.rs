use std::path::PathBuf;

use lanzaboote_tool::architecture::Architecture;

/// Systemd-specific architecture helpers
pub trait SystemdArchitectureExt {
    fn systemd_filename(&self) -> PathBuf;
}

impl SystemdArchitectureExt for Architecture {
    fn systemd_filename(&self) -> PathBuf {
        format!("systemd-boot{}.efi", self.efi_representation()).into()
    }
}
