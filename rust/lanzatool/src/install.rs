use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use nix::unistd::sync;
use tempfile::tempdir;

use crate::esp::EspPaths;
use crate::gc::Roots;
use crate::generation::{Generation, GenerationLink};
use crate::pe;
use crate::signature::KeyPair;

pub struct Installer {
    gc_roots: Roots,
    lanzaboote_stub: PathBuf,
    key_pair: KeyPair,
    configuration_limit: usize,
    esp: PathBuf,
    generation_links: Vec<PathBuf>,
}

impl Installer {
    pub fn new(
        lanzaboote_stub: PathBuf,
        key_pair: KeyPair,
        configuration_limit: usize,
        esp: PathBuf,
        generation_links: Vec<PathBuf>,
    ) -> Self {
        Self {
            gc_roots: Roots::new(),
            lanzaboote_stub,
            key_pair,
            configuration_limit,
            esp,
            generation_links,
        }
    }

    pub fn install(&mut self) -> Result<()> {
        let mut links = self
            .generation_links
            .iter()
            .map(GenerationLink::from_path)
            .collect::<Result<Vec<GenerationLink>>>()?;

        // A configuration limit of 0 means there is no limit.
        if self.configuration_limit > 0 {
            // Sort the links by version.
            links.sort_by_key(|l| l.version);

            // Only install the number of generations configured.
            links = links
                .into_iter()
                .rev()
                .take(self.configuration_limit)
                .collect()
        };
        self.install_links(links)?;

        self.gc_roots.collect_garbage(&self.esp)?;

        Ok(())
    }

    fn install_links(&mut self, links: Vec<GenerationLink>) -> Result<()> {
        for link in links {
            let generation_result = Generation::from_link(&link)
                .with_context(|| format!("Failed to build generation from link: {link:?}"));

            // Ignore failing to read a generation so that old malformed generations do not stop
            // lanzatool from working.
            let generation = match generation_result {
                Ok(generation) => generation,
                Err(e) => {
                    println!("Malformed generation: {:?}", e);
                    continue;
                }
            };

            println!("Installing generation {generation}");

            self.install_generation(&generation)
                .context("Failed to install generation")?;

            for (name, bootspec) in &generation.spec.bootspec.specialisation {
                let specialised_generation = generation.specialise(name, bootspec)?;

                println!("Installing specialisation: {name} of generation: {generation}");

                self.install_generation(&specialised_generation)
                    .context("Failed to install specialisation")?;
            }
        }
        Ok(())
    }

    fn install_generation(&mut self, generation: &Generation) -> Result<()> {
        let bootspec = &generation.spec.bootspec;
        let secureboot_extensions = &generation.spec.extensions;

        let esp_paths = EspPaths::new(&self.esp, generation)?;
        self.gc_roots.extend(esp_paths.to_iter());

        let kernel_cmdline =
            assemble_kernel_cmdline(&bootspec.init, bootspec.kernel_params.clone());

        // prepare a secure temporary directory
        // permission bits are not set, because when files below
        // are opened, they are opened with 600 mode bits.
        // hence, they cannot be read except by the current user
        // which is assumed to be root in most cases.
        // TODO(Raito): prove to niksnur this is actually acceptable.
        let secure_temp_dir = tempdir()?;

        println!("Appending secrets to initrd...");

        let initrd_location = secure_temp_dir.path().join("initrd");
        copy(
            bootspec
                .initrd
                .as_ref()
                .context("Lanzaboote does not support missing initrd yet")?,
            &initrd_location,
        )?;
        if let Some(initrd_secrets_script) = &bootspec.initrd_secrets {
            append_initrd_secrets(initrd_secrets_script, &initrd_location)?;
        }

        let systemd_boot = bootspec
            .toplevel
            .0
            .join("systemd/lib/systemd/boot/efi/systemd-bootx64.efi");

        [
            (&systemd_boot, &esp_paths.efi_fallback),
            (&systemd_boot, &esp_paths.systemd_boot),
            (&bootspec.kernel, &esp_paths.kernel),
        ]
        .into_iter()
        .try_for_each(|(from, to)| install_signed(&self.key_pair, from, to))?;

        // The initrd doesn't need to be signed. Lanzaboote has its
        // hash embedded and will refuse loading it when the hash
        // mismatches.
        install(&initrd_location, &esp_paths.initrd).context("Failed to install initrd to ESP")?;

        let lanzaboote_image = pe::lanzaboote_image(
            &secure_temp_dir,
            &self.lanzaboote_stub,
            &secureboot_extensions.os_release,
            &kernel_cmdline,
            &esp_paths.kernel,
            &esp_paths.initrd,
            &esp_paths.esp,
        )
        .context("Failed to assemble stub")?;

        install_signed(
            &self.key_pair,
            &lanzaboote_image,
            &esp_paths.lanzaboote_image,
        )
        .context("Failed to install lanzaboote")?;

        // Sync files to persistent storage. This may improve the
        // chance of a consistent boot directory in case the system
        // crashes.
        sync();

        println!(
            "Successfully installed lanzaboote to '{}'",
            esp_paths.esp.display()
        );

        Ok(())
    }
}

/// Install a PE file. The PE gets signed in the process.
///
/// The file is only signed and copied if it doesn't exist at the destination
fn install_signed(key_pair: &KeyPair, from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        println!("{} already exists, skipping...", to.display());
    } else {
        println!("Signing and installing {}...", to.display());
        ensure_parent_dir(to);
        key_pair
            .sign_and_copy(from, to)
            .with_context(|| format!("Failed to copy and sign file from {:?} to {:?}", from, to))?;
    }

    Ok(())
}

/// Install an arbitrary file
///
/// The file is only copied if it doesn't exist at the destination
fn install(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        println!("{} already exists, skipping...", to.display());
    } else {
        println!("Installing {}...", to.display());
        ensure_parent_dir(to);
        copy(from, to)?;
    }

    Ok(())
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
