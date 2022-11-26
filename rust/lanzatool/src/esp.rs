use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::generation::Generation;

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
    pub fn new(esp: impl AsRef<Path>, generation: &Generation) -> Result<Self> {
        let esp = esp.as_ref();
        let esp_nixos = esp.join("EFI/nixos");
        let esp_linux = esp.join("EFI/Linux");
        let esp_systemd = esp.join("EFI/systemd");
        let esp_efi_fallback_dir = esp.join("EFI/BOOT");

        let bootspec = &generation.bootspec;

        Ok(Self {
            esp: esp.to_path_buf(),
            nixos: esp_nixos.clone(),
            kernel: esp_nixos.join(nixos_path(&bootspec.kernel, "bzImage")?),
            initrd: esp_nixos.join(nixos_path(&bootspec.initrd, "initrd")?),
            linux: esp_linux.clone(),
            lanzaboote_image: esp_linux.join(generation_path(generation)),
            efi_fallback_dir: esp_efi_fallback_dir.clone(),
            efi_fallback: esp_efi_fallback_dir.join("BOOTX64.EFI"),
            systemd: esp_systemd.clone(),
            systemd_boot: esp_systemd.join("systemd-bootx64.efi"),
        })
    }
}

fn nixos_path(path: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
    let resolved = path
        .as_ref()
        .read_link()
        .unwrap_or_else(|_| path.as_ref().into());

    let parent = resolved.parent().ok_or_else(|| {
        anyhow::anyhow!(format!(
            "Path: {} does not have a parent",
            resolved.display()
        ))
    })?;

    let without_store = parent.strip_prefix("/nix/store").with_context(|| {
        format!(
            "Failed to strip /nix/store from path {}",
            path.as_ref().display()
        )
    })?;

    let nixos_filename = format!("{}-{}.efi", without_store.display(), name);

    Ok(PathBuf::from(nixos_filename))
}

fn generation_path(generation: &Generation) -> PathBuf {
    PathBuf::from(format!("nixos-generation-{}.efi", generation))
}
