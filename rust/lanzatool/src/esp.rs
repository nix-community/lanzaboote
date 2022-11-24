use std::path::{Path, PathBuf};

pub struct EspPaths {
    pub esp: PathBuf,
    pub nixos: PathBuf,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub linux: PathBuf,
    pub lanzaboote_image: PathBuf,
    pub efi_fallback_dir: PathBuf,
    pub efi_fallback: PathBuf,
    pub systemd: PathBuf,
    pub systemd_boot: PathBuf,
}

impl EspPaths {
    pub fn new(esp: &str) -> Self {
        let esp = Path::new(esp);
        let esp_nixos = esp.join("EFI/nixos");
        let esp_linux = esp.join("EFI/Linux");
        let esp_systemd = esp.join("EFI/systemd");
        let esp_efi_fallback_dir = esp.join("EFI/BOOT");

        Self {
            esp: esp.to_owned(),
            nixos: esp_nixos.clone(),
            kernel: esp_nixos.join("kernel"),
            initrd: esp_nixos.join("initrd"),
            linux: esp_linux.clone(),
            lanzaboote_image: esp_linux.join("lanzaboote-image.efi"),
            efi_fallback_dir: esp_efi_fallback_dir.clone(),
            efi_fallback: esp_efi_fallback_dir.join("BOOTX64.EFI"),
            systemd: esp_systemd.clone(),
            systemd_boot: esp_systemd.join("systemd-bootx64.efi"),
        }
    }
}
