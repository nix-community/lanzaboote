use std::path::{Path, PathBuf};

use crate::architecture::SystemdArchitectureExt;
use lanzaboote_tool::architecture::Architecture;
use lanzaboote_tool::esp::EspPaths;

/// Paths to the boot files that are not specific to a generation.
/// Systemd variant
pub struct SystemdEspPaths {
    pub esp: PathBuf,
    pub efi: PathBuf,
    pub nixos: PathBuf,
    pub linux: PathBuf,
    pub efi_fallback_dir: PathBuf,
    pub efi_fallback: PathBuf,
    pub systemd: PathBuf,
    pub systemd_boot: PathBuf,
    pub loader: PathBuf,
    pub systemd_boot_loader_config: PathBuf,
}

impl EspPaths<10> for SystemdEspPaths {
    fn new(esp: impl AsRef<Path>, boot: impl AsRef<Path>, architecture: Architecture) -> Self {
        let esp = esp.as_ref();
        let boot = boot.as_ref();
        let esp_efi = esp.join("EFI");
        let boot_efi = boot.join("EFI");
        let boot_efi_nixos = boot_efi.join("nixos");
        let boot_efi_linux = boot_efi.join("Linux");
        let esp_efi_systemd = esp_efi.join("systemd");
        let esp_efi_efi_fallback_dir = esp_efi.join("BOOT");
        let loader = esp.join("loader");
        let systemd_boot_loader_config = loader.join("loader.conf");

        Self {
            esp: esp.to_path_buf(),
            efi: esp_efi,
            nixos: boot_efi_nixos,
            linux: boot_efi_linux,
            efi_fallback_dir: esp_efi_efi_fallback_dir.clone(),
            efi_fallback: esp_efi_efi_fallback_dir.join(architecture.efi_fallback_filename()),
            systemd: esp_efi_systemd.clone(),
            systemd_boot: esp_efi_systemd.join(architecture.systemd_filename()),
            loader,
            systemd_boot_loader_config,
        }
    }

    fn nixos_path(&self) -> &Path {
        &self.nixos
    }

    fn linux_path(&self) -> &Path {
        &self.linux
    }

    fn iter(&self) -> std::array::IntoIter<&PathBuf, 10> {
        [
            &self.esp,
            &self.efi,
            &self.nixos,
            &self.linux,
            &self.efi_fallback_dir,
            &self.efi_fallback,
            &self.systemd,
            &self.systemd_boot,
            &self.loader,
            &self.systemd_boot_loader_config,
        ]
        .into_iter()
    }
}
