use std::array::IntoIter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::generation::Generation;

pub struct EspPaths {
    pub esp: PathBuf,
    pub efi: PathBuf,
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
        let efi = esp.join("EFI");
        let efi_nixos = efi.join("nixos");
        let efi_linux = efi.join("Linux");
        let efi_systemd = efi.join("systemd");
        let efi_efi_fallback_dir = efi.join("BOOT");

        let bootspec = &generation.spec.bootspec;

        Ok(Self {
            esp: esp.to_path_buf(),
            efi,
            nixos: efi_nixos.clone(),
            kernel: efi_nixos.join(nixos_path(&bootspec.kernel, "bzImage")?),
            initrd: efi_nixos.join(nixos_path(
                bootspec
                    .initrd
                    .as_ref()
                    .context("Lanzaboote does not support missing initrd yet")?,
                "initrd",
            )?),
            linux: efi_linux.clone(),
            lanzaboote_image: efi_linux.join(generation_path(generation)),
            efi_fallback_dir: efi_efi_fallback_dir.clone(),
            efi_fallback: efi_efi_fallback_dir.join("BOOTX64.EFI"),
            systemd: efi_systemd.clone(),
            systemd_boot: efi_systemd.join("systemd-bootx64.efi"),
        })
    }

    /// Return the used file paths to store as garbage collection roots.
    pub fn to_iter(&self) -> IntoIter<&PathBuf, 11> {
        [
            &self.esp,
            &self.efi,
            &self.nixos,
            &self.kernel,
            &self.initrd,
            &self.linux,
            &self.lanzaboote_image,
            &self.efi_fallback_dir,
            &self.efi_fallback,
            &self.systemd,
            &self.systemd_boot,
        ]
        .into_iter()
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
    if let Some(specialisation_name) = generation.is_specialised() {
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
