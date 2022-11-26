use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use nix::unistd::sync;
use tempfile::tempdir;

use crate::esp::EspPaths;
use crate::generation::Generation;
use crate::pe;
use crate::signature::KeyPair;

pub struct Installer {
    lanzaboote_stub: PathBuf,
    initrd_stub: PathBuf,
    key_pair: KeyPair,
    _pki_bundle: Option<PathBuf>,
    _auto_enroll: bool,
    esp: PathBuf,
    generations: Vec<PathBuf>,
}

impl Installer {
    pub fn new(
        lanzaboote_stub: PathBuf,
        initrd_stub: PathBuf,
        key_pair: KeyPair,
        _pki_bundle: Option<PathBuf>,
        _auto_enroll: bool,
        esp: PathBuf,
        generations: Vec<PathBuf>,
    ) -> Self {
        Self {
            lanzaboote_stub,
            initrd_stub,
            key_pair,
            _pki_bundle,
            _auto_enroll,
            esp,
            generations,
        }
    }

    pub fn install(&self) -> Result<()> {
        for toplevel in &self.generations {
            let generation = Generation::from_toplevel(toplevel).with_context(|| {
                format!("Failed to build generation from toplevel: {toplevel:?}")
            })?;

            println!("Installing generation {generation}");

            self.install_generation(generation)?
        }

        Ok(())
    }

    pub fn install_generation(&self, generation: Generation) -> Result<()> {
        println!("Reading bootspec...");

        let bootspec = &generation.bootspec;

        let esp_paths = EspPaths::new(&self.esp, &generation)?;

        println!("Assembling lanzaboote image...");

        let kernel_cmdline =
            assemble_kernel_cmdline(&bootspec.init, bootspec.kernel_params.clone());

        // prepare a secure temporary directory
        // permission bits are not set, because when files below
        // are opened, they are opened with 600 mode bits.
        // hence, they cannot be read except by the current user
        // which is assumed to be root in most cases.
        // TODO(Raito): prove to niksnur this is actually acceptable.
        let secure_temp_dir = tempdir()?;

        let lanzaboote_image = pe::lanzaboote_image(
            &secure_temp_dir,
            &self.lanzaboote_stub,
            &bootspec.extension.os_release,
            &kernel_cmdline,
            &esp_paths.kernel,
            &esp_paths.initrd,
            &esp_paths.esp,
        )
        .context("Failed to assemble stub")?;

        println!("Wrapping initrd into a PE binary...");

        let initrd_location = secure_temp_dir.path().join("initrd");
        copy(&bootspec.initrd, &initrd_location)?;
        if let Some(initrd_secrets_script) = &bootspec.initrd_secrets {
            append_initrd_secrets(initrd_secrets_script, &initrd_location)?;
        }
        let wrapped_initrd = pe::wrap_initrd(&secure_temp_dir, &self.initrd_stub, &initrd_location)
            .context("Failed to assemble stub")?;

        println!("Sign and copy files to EFI system partition...");

        let systemd_boot = bootspec
            .toplevel
            .join("systemd/lib/systemd/boot/efi/systemd-bootx64.efi");

        let files_to_copy_and_sign = [
            (&systemd_boot, &esp_paths.efi_fallback),
            (&systemd_boot, &esp_paths.systemd_boot),
            (&lanzaboote_image, &esp_paths.lanzaboote_image),
            (&bootspec.kernel, &esp_paths.kernel),
            (&wrapped_initrd, &esp_paths.initrd),
        ];

        for (from, to) in files_to_copy_and_sign {
            println!("Signing {}...", to.display());

            ensure_parent_dir(to);
            self.key_pair.sign_and_copy(from, to).with_context(|| {
                format!("Failed to copy and sign file from {:?} to {:?}", from, to)
            })?;
            // Call sync to improve the likelihood that file is actually written to disk
            sync();
        }

        println!(
            "Successfully installed lanzaboote to '{}'",
            esp_paths.esp.display()
        );

        Ok(())
    }
}

pub fn append_initrd_secrets(
    append_initrd_secrets_path: &Path,
    initrd_path: &PathBuf,
) -> Result<()> {
    let status = Command::new(append_initrd_secrets_path)
        .args(vec![initrd_path])
        .status()
        .context("Failed to append initrd secrets")?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to append initrd secrets with args `{:?}`",
            vec![append_initrd_secrets_path, initrd_path]
        ));
    }

    Ok(())
}

fn assemble_kernel_cmdline(init: &Path, kernel_params: Vec<String>) -> Vec<String> {
    let init_string = String::from(
        init.to_str()
            .expect("Failed to convert init path to string"),
    );
    let mut kernel_cmdline: Vec<String> = vec![format!("init={}", init_string)];
    kernel_cmdline.extend(kernel_params);
    kernel_cmdline
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    ensure_parent_dir(to);
    fs::copy(from, to)
        .with_context(|| format!("Failed to copy from {} to {}", from.display(), to.display()))?;

    // Set permission of all files copied to 0o755
    let mut perms = fs::metadata(to)
        .with_context(|| format!("File {} doesn't have metadata", to.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(to, perms)
        .with_context(|| format!("Failed to set permissions to: {}", to.display()))?;

    Ok(())
}

// Ensures the parent directory of an arbitrary path exists
fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
}
