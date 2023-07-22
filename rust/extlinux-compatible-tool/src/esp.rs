use std::path::{Path, PathBuf};

use lanzaboote_tool::esp::EspPaths;

/// Paths to the boot files that are not specific to a generation.
/// Extlinux-compatible variant.
pub struct ExtlinuxEspPaths {
    pub esp: PathBuf,
    pub efi: PathBuf,
    pub nixos: PathBuf,
    pub extlinux: PathBuf,
    pub efi_fallback_dir: PathBuf,
    pub efi_fallback: PathBuf,
    pub loader: PathBuf,
    pub extlinux_config: PathBuf,
}

impl EspPaths<7> for ExtlinuxEspPaths {
    fn new(esp: impl AsRef<Path>) -> Self {
        let esp = esp.as_ref();
        let efi = esp.join("EFI");
        let efi_nixos = efi.join("nixos");
        let efi_extlinux = efi.join("extlinux");
        let efi_efi_fallback_dir = efi.join("BOOT");
        let loader = esp.join("loader");
        let extlinux_config = efi_extlinux.join("extlinux.conf");

        Self {
            esp: esp.to_path_buf(),
            efi,
            nixos: efi_nixos,
            extlinux: efi_extlinux,
            efi_fallback_dir: efi_efi_fallback_dir.clone(),
            efi_fallback: efi_efi_fallback_dir.join("BOOTX64.EFI"),
            loader,
            extlinux_config,
        }
    }

    fn nixos_path(&self) -> &Path {
        &self.nixos
    }

    fn linux_path(&self) -> &Path {
        &self.extlinux
    }

    fn iter(&self) -> std::array::IntoIter<&PathBuf, 7> {
        [
            &self.esp,
            &self.efi,
            &self.nixos,
            &self.extlinux,
            &self.efi_fallback_dir,
            &self.efi_fallback,
            &self.loader,
        ].into_iter()
    }
}
