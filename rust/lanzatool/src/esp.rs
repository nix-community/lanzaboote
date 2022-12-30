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

        let bootspec = &generation.spec.bootspec;

        Ok(Self {
            esp: esp.to_path_buf(),
            nixos: esp_nixos.clone(),
            kernel: esp_nixos.join(nixos_path(&bootspec.kernel, "bzImage")?),
            initrd: esp_nixos.join(nixos_path(
                bootspec
                    .initrd
                    .as_ref()
                    .context("Lanzaboote does not support missing initrd yet")?,
                "initrd",
            )?),
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

    let parent_final_component = resolved
        .parent()
        .and_then(|x| x.file_name())
        .and_then(|x| x.to_str())
        .with_context(|| format!("Failed to extract final component from: {:?}", resolved))?;

    let nixos_filename = format!("{}-{}.efi", parent_final_component, name);

    Ok(PathBuf::from(nixos_filename))
}

fn generation_path(generation: &Generation) -> PathBuf {
    if let Some(specialisation_name) = generation.is_specialized() {
        PathBuf::from(format!(
            "nixos-generation-{}-specialisation-{}.efi",
            generation, specialisation_name
        ))
    } else {
        PathBuf::from(format!("nixos-generation-{}.efi", generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nixos_path_creates_correct_filename_from_nix_store_path() -> Result<()> {
        let path =
            Path::new("/nix/store/xqplddjjjy1lhzyzbcv4dza11ccpcfds-initrd-linux-6.1.1/initrd");

        let generated_filename = nixos_path(path, "initrd")?;

        let expected_filename =
            PathBuf::from("xqplddjjjy1lhzyzbcv4dza11ccpcfds-initrd-linux-6.1.1-initrd.efi");

        assert_eq!(generated_filename, expected_filename);
        Ok(())
    }
}
