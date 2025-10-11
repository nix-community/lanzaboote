use std::path::{Path, PathBuf};

use crate::architecture::RefindArchitectureExt;
use lanzaboote_tool::architecture::Architecture;
use lanzaboote_tool::esp::EspPaths;

/// Paths to the boot files that are not specific to a generation.
/// rEFInd variant
pub struct RefindEspPaths {
    pub esp: PathBuf,
    pub efi: PathBuf,
    pub nixos: PathBuf,
    pub linux: PathBuf,
    pub efi_fallback_dir: PathBuf,
    pub efi_fallback: PathBuf,
    pub refind: PathBuf,
    pub refind_binary: PathBuf,
    pub refind_config: PathBuf,
}

impl EspPaths<9> for RefindEspPaths {
    fn new(esp: impl AsRef<Path>, architecture: Architecture) -> Self {
        let esp = esp.as_ref();
        let efi = esp.join("EFI");
        let efi_nixos = efi.join("nixos");
        let efi_linux = efi.join("Linux");
        let efi_refind = efi.join("refind");
        let efi_efi_fallback_dir = efi.join("BOOT");

        Self {
            esp: esp.to_path_buf(),
            efi,
            nixos: efi_nixos,
            linux: efi_linux,
            efi_fallback_dir: efi_efi_fallback_dir.clone(),
            efi_fallback: efi_efi_fallback_dir.join(architecture.efi_fallback_filename()),
            refind: efi_refind.clone(),
            refind_binary: efi_refind.join(architecture.refind_filename()),
            refind_config: efi_refind.join("refind.conf"),
        }
    }

    fn nixos_path(&self) -> &Path {
        &self.nixos
    }

    fn linux_path(&self) -> &Path {
        &self.linux
    }

    fn iter(&self) -> std::array::IntoIter<&PathBuf, 9> {
        [
            &self.esp,
            &self.efi,
            &self.nixos,
            &self.linux,
            &self.efi_fallback_dir,
            &self.efi_fallback,
            &self.refind,
            &self.refind_binary,
            &self.refind_config,
        ]
        .into_iter()
    }
}
