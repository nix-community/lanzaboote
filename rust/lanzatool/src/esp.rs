use std::path::{Path, PathBuf};

pub struct EspPaths {
    pub esp: PathBuf,
    pub nixos: PathBuf,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
}

impl EspPaths {
    pub fn new(esp: &str) -> Self {
        let esp = Path::new(esp);
        let esp_nixos = esp.join("EFI/nixos");

        Self {
            esp: esp.to_owned(),
            nixos: esp_nixos.clone(),
            kernel: esp_nixos.join("kernel"),
            initrd: esp_nixos.join("initrd"),
        }
    }
}
