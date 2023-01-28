use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use nix::unistd::sync;

use crate::esp::{EspGenerationPaths, EspPaths};
use crate::gc::Roots;
use crate::generation::{Generation, GenerationLink};
use crate::os_release::OsRelease;
use crate::pe;
use crate::signature::KeyPair;
use crate::systemd::SystemdVersion;
use crate::utils::SecureTempDirExt;

pub struct Installer {
    gc_roots: Roots,
    lanzaboote_stub: PathBuf,
    systemd: PathBuf,
    key_pair: KeyPair,
    configuration_limit: usize,
    esp_paths: EspPaths,
    generation_links: Vec<PathBuf>,
}

impl Installer {
    pub fn new(
        lanzaboote_stub: PathBuf,
        systemd: PathBuf,
        key_pair: KeyPair,
        configuration_limit: usize,
        esp: PathBuf,
        generation_links: Vec<PathBuf>,
    ) -> Self {
        let mut gc_roots = Roots::new();
        let esp_paths = EspPaths::new(esp);
        gc_roots.extend(esp_paths.to_iter());

        Self {
            gc_roots,
            lanzaboote_stub,
            systemd,
            key_pair,
            configuration_limit,
            esp_paths,
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

        self.install_systemd_boot()?;

        // Only collect garbage in these two directories. This way, no files that do not belong to
        // the NixOS installation are deleted. Lanzatool takes full control over the esp/EFI/nixos
        // directory and deletes ALL files that it doesn't know about. Dual- or multiboot setups
        // that need files in this directory will NOT work.
        self.gc_roots.collect_garbage(&self.esp_paths.nixos)?;
        // The esp/EFI/Linux directory is assumed to be potentially shared with other distros.
        // Thus, only files that start with "nixos-" are garbage collected (i.e. potentially
        // deleted).
        self.gc_roots
            .collect_garbage_with_filter(&self.esp_paths.linux, |p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.starts_with("nixos-"))
            })?;

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

        let esp_gen_paths = EspGenerationPaths::new(&self.esp_paths, generation)?;
        self.gc_roots.extend(esp_gen_paths.to_iter());

        let kernel_cmdline =
            assemble_kernel_cmdline(&bootspec.init, bootspec.kernel_params.clone());

        // This tempdir must live for the entire lifetime of the current function.
        let tempdir = tempfile::tempdir()?;

        let os_release = OsRelease::from_generation(generation)
            .context("Failed to build OsRelease from generation.")?;
        let os_release_path = tempdir
            .write_secure_file("os-release", os_release.to_string().as_bytes())
            .context("Failed to write os-release file.")?;

        println!("Appending secrets to initrd...");

        let initrd_location = tempdir.path().join("initrd");
        fs::copy(
            bootspec
                .initrd
                .as_ref()
                .context("Lanzaboote does not support missing initrd yet")?,
            &initrd_location,
        )?;
        if let Some(initrd_secrets_script) = &bootspec.initrd_secrets {
            append_initrd_secrets(initrd_secrets_script, &initrd_location)?;
        }

        // The initrd doesn't need to be signed. The stub has its hash embedded and will refuse
        // loading it when the hash mismatches.
        //
        // The initrd and kernel are not forcibly installed because they are not built
        // reproducibly. Forcibly installing (i.e. overwriting) them is likely to break older
        // generations that point to the same initrd/kernel because the hash embedded in the stub
        // will not match anymore.
        install(&initrd_location, &esp_gen_paths.initrd)
            .context("Failed to install initrd to ESP")?;
        install_signed(&self.key_pair, &bootspec.kernel, &esp_gen_paths.kernel)
            .context("Failed to install kernel to ESP.")?;

        let lanzaboote_image = pe::lanzaboote_image(
            &tempdir,
            &self.lanzaboote_stub,
            &os_release_path,
            &kernel_cmdline,
            &esp_gen_paths.kernel,
            &esp_gen_paths.initrd,
            &self.esp_paths.esp,
        )
        .context("Failed to assemble stub")?;

        install_signed(
            &self.key_pair,
            &lanzaboote_image,
            &esp_gen_paths.lanzaboote_image,
        )
        .context("Failed to install lanzaboote")?;

        // Sync files to persistent storage. This may improve the
        // chance of a consistent boot directory in case the system
        // crashes.
        sync();

        println!(
            "Successfully installed lanzaboote to '{}'",
            self.esp_paths.esp.display()
        );

        Ok(())
    }

    /// Install systemd-boot to ESP.
    ///
    /// systemd-boot is only updated when a newer version is available OR when the currently
    /// installed version is not signed. This enables switching to Lanzaboote without having to
    /// manually delete previous unsigned systemd-boot binaries and minimizes the number of writes
    /// to the ESP.
    ///
    /// Checking for the version also allows us to skip buggy systemd versions in the future.
    fn install_systemd_boot(&self) -> Result<()> {
        let systemd_boot = self
            .systemd
            .join("lib/systemd/boot/efi/systemd-bootx64.efi");

        let paths = [
            (&systemd_boot, &self.esp_paths.efi_fallback),
            (&systemd_boot, &self.esp_paths.systemd_boot),
        ];

        for (from, to) in paths {
            if newer_systemd_boot(from, to)? || !&self.key_pair.verify(to) {
                force_install_signed(&self.key_pair, from, to)
                    .with_context(|| format!("Failed to install systemd-boot binary to: {to:?}"))?;
            }
        }
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
        force_install_signed(key_pair, from, to)?;
    }

    Ok(())
}

/// Sign and forcibly install a PE file.
///
/// If the file already exists at the destination, it is overwritten.
///
/// This is implemented as an atomic write. The file is first written to the destination with a
/// `.tmp` suffix and then renamed to its final name. This is atomic, because a rename is an atomic
/// operation on POSIX platforms.
fn force_install_signed(key_pair: &KeyPair, from: &Path, to: &Path) -> Result<()> {
    println!("Signing and installing {}...", to.display());
    let to_tmp = to.with_extension(".tmp");
    ensure_parent_dir(&to_tmp);
    key_pair
        .sign_and_copy(from, &to_tmp)
        .with_context(|| format!("Failed to copy and sign file from {from:?} to {to:?}"))?;
    fs::rename(&to_tmp, to).with_context(|| {
        format!("Failed to move temporary file {to_tmp:?} to final location {to:?}")
    })?;
    Ok(())
}

/// Install an arbitrary file.
///
/// The file is only copied if it doesn't exist at the destination.
///
/// This function is only designed to copy files to the ESP. It sets the permission bits of the
/// file at the destination to 0o755, the expected permissions for a vfat ESP. This is useful for
/// producing file systems trees which can then be converted to a file system image.
fn install(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        println!("{} already exists, skipping...", to.display());
    } else {
        println!("Installing {}...", to.display());
        ensure_parent_dir(to);
        atomic_copy(from, to)?;
        set_permission_bits(to, 0o755)
            .with_context(|| format!("Failed to set permission bits to 0o755 on file: {to:?}"))?;
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

/// Atomically copy a file.
///
/// The file is first written to the destination with a `.tmp` suffix and then renamed to its final
/// name. This is atomic, because a rename is an atomic operation on POSIX platforms.
fn atomic_copy(from: &Path, to: &Path) -> Result<()> {
    let to_tmp = to.with_extension(".tmp");

    fs::copy(from, &to_tmp)
        .with_context(|| format!("Failed to copy from {from:?} to {to_tmp:?}",))?;

    fs::rename(&to_tmp, to).with_context(|| {
        format!("Failed to move temporary file {to_tmp:?} to final location {to:?}")
    })
}

/// Set the octal permission bits of the specified file.
fn set_permission_bits(path: &Path, permission_bits: u32) -> Result<()> {
    let mut perms = fs::metadata(path)
        .with_context(|| format!("File {path:?} doesn't have any metadata"))?
        .permissions();
    perms.set_mode(permission_bits);
    fs::set_permissions(path, perms)
        .with_context(|| format!("Failed to set permissions on {path:?}"))
}

// Ensures the parent directory of an arbitrary path exists
fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
}

/// Determine if a newer systemd-boot version is available.
///
/// "Newer" can mean
///   (1) no file exists at the destination,
///   (2) the file at the destination is malformed,
///   (3) a binary with a higher version is available.
fn newer_systemd_boot(from: &Path, to: &Path) -> Result<bool> {
    // If the file doesn't exists at the destination, it should be installed.
    if !to.exists() {
        return Ok(true);
    }

    // If the version from the source binary cannot be read, something is irrecoverably wrong.
    let from_version = SystemdVersion::from_systemd_boot_binary(from)
        .with_context(|| format!("Failed to read systemd-boot version from {from:?}."))?;

    // If the version cannot be read from the destination binary, it is malformed. It should be
    // forcibly reinstalled.
    let to_version = match SystemdVersion::from_systemd_boot_binary(to) {
        Ok(version) => version,
        _ => return Ok(true),
    };

    Ok(from_version > to_version)
}
