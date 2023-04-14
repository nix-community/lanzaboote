use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::ToString;

use anyhow::{anyhow, Context, Result};
use nix::unistd::sync;
use tempfile::TempDir;

use crate::esp::{EspGenerationPaths, EspPaths};
use crate::gc::Roots;
use crate::generation::{Generation, GenerationLink};
use crate::os_release::OsRelease;
use crate::pe;
use crate::signature::KeyPair;
use crate::systemd::SystemdVersion;
use crate::utils::{file_hash, SecureTempDirExt};

pub struct Installer {
    broken_gens: BTreeSet<u64>,
    gc_roots: Roots,
    lanzaboote_stub: PathBuf,
    systemd: PathBuf,
    systemd_boot_loader_config: PathBuf,
    key_pair: KeyPair,
    configuration_limit: usize,
    esp_paths: EspPaths,
    generation_links: Vec<PathBuf>,
}

impl Installer {
    pub fn new(
        lanzaboote_stub: PathBuf,
        systemd: PathBuf,
        systemd_boot_loader_config: PathBuf,
        key_pair: KeyPair,
        configuration_limit: usize,
        esp: PathBuf,
        generation_links: Vec<PathBuf>,
    ) -> Self {
        let mut gc_roots = Roots::new();
        let esp_paths = EspPaths::new(esp);
        gc_roots.extend(esp_paths.to_iter());

        Self {
            broken_gens: BTreeSet::new(),
            gc_roots,
            lanzaboote_stub,
            systemd,
            systemd_boot_loader_config,
            key_pair,
            configuration_limit,
            esp_paths,
            generation_links,
        }
    }

    pub fn install(&mut self) -> Result<()> {
        log::info!("Installing Lanzaboote to {:?}...", self.esp_paths.esp);

        let mut links = self
            .generation_links
            .iter()
            .map(GenerationLink::from_path)
            .collect::<Result<Vec<GenerationLink>>>()?;

        // Sort the links by version. The links need to always be sorted to ensure the secrets of
        // the latest generation are appended to the initrd when multiple generations point to the
        // same initrd.
        links.sort_by_key(|l| l.version);

        // A configuration limit of 0 means there is no limit.
        if self.configuration_limit > 0 {
            // Only install the number of generations configured. Reverse the list to only take the
            // latest generations and then, after taking them, reverse the list again so that the
            // generations are installed from oldest to newest, i.e. from smallest to largest
            // generation version.
            links = links
                .into_iter()
                .rev()
                .take(self.configuration_limit)
                .rev()
                .collect()
        };
        self.install_generations_from_links(&links)?;

        self.install_systemd_boot()?;

        if self.broken_gens.is_empty() {
            log::info!("Collecting garbage...");
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
        } else {
            // This might produce a ridiculous message if you have a lot of malformed generations.
            let warning = indoc::formatdoc! {"
                Garbage collection is disabled because you have malformed NixOS generations that do
                not contain a readable bootspec document.

                Remove the malformed generations to re-enable garbage collection with 
                `nix-env --delete-generations {}` 
            ", self.broken_gens.iter().map(ToString::to_string).collect::<Vec<String>>().join(" ")};
            log::warn!("{warning}");
        };

        log::info!("Successfully installed Lanzaboote.");
        Ok(())
    }

    /// Install all generations from the provided `GenerationLinks`.
    ///
    /// Iterates over the links twice:
    ///     (1) First, building all unsigned artifacts and storing the mapping from source to
    ///     destination in `GenerationArtifacts`. `GenerationArtifacts` ensures that there are no
    ///     duplicate destination paths and thus ensures that the hashes embedded in the lanzaboote
    ///     image do not get invalidated because the files to which they point get overwritten by a
    ///     later generation.
    ///     (2) Second, building all signed artifacts using the previously built mapping from source to
    ///     destination in the `GenerationArtifacts`.
    ///
    /// This way, in the second step, all paths and thus all hashes for all generations are already
    /// known. The signed files can now be constructed with known good hashes **across** all
    /// generations.
    fn install_generations_from_links(&mut self, links: &[GenerationLink]) -> Result<()> {
        // This struct must live for the entire lifetime of this function so that the contained
        // tempdir does not go out of scope and thus does not get deleted.
        let mut generation_artifacts =
            GenerationArtifacts::new().context("Failed to create GenerationArtifacts.")?;

        self.build_generation_artifacts_from_links(
            &mut generation_artifacts,
            links,
            Self::build_unsigned_generation_artifacts,
        )
        .context("Failed to build unsigned generation artifacts.")?;

        self.build_generation_artifacts_from_links(
            &mut generation_artifacts,
            links,
            Self::build_signed_generation_artifacts,
        )
        .context("Failed to build signed generation artifacts.")?;

        generation_artifacts
            .install(&self.key_pair)
            .context("Failed to install files.")?;

        // Sync files to persistent storage. This may improve the
        // chance of a consistent boot directory in case the system
        // crashes.
        sync();

        Ok(())
    }

    /// Build all generation artifacts from a list of `GenerationLink`s.
    ///
    /// This function accepts a closure to build the generation artifacts for a single generation.
    fn build_generation_artifacts_from_links<F>(
        &mut self,
        generation_artifacts: &mut GenerationArtifacts,
        links: &[GenerationLink],
        mut build_generation_artifacts: F,
    ) -> Result<()>
    where
        F: FnMut(&mut Self, &Generation, &mut GenerationArtifacts) -> Result<()>,
    {
        let generations = links
            .iter()
            .filter_map(|link| {
                let generation_result = Generation::from_link(link)
                    .with_context(|| format!("Failed to build generation from link: {link:?}"));

                // Ignore failing to read a generation so that old malformed generations do not stop
                // lzbt from working.
                if generation_result.is_err() {
                    // If there is ANY malformed generation present, completely disable all garbage
                    // collection to protect the old generations from being deleted. The user has
                    // to manually intervene by getting rid of the old generations to re-enable
                    // garbage collection. This safeguard against catastrophic failure in case of
                    // unhandled upstream changes to NixOS.
                    self.broken_gens.insert(link.version);
                }

                generation_result.ok()
            })
            .collect::<Vec<Generation>>();

        if generations.is_empty() {
            // We can't continue, because we would remove all boot entries, if we did.
            return Err(anyhow!("No bootable generations found! Aborting to avoid unbootable system. Please check for Lanzaboote updates!"));
        }

        for generation in generations {
            build_generation_artifacts(self, &generation, generation_artifacts)
                .context("Failed to build generation artifacts.")?;

            for (name, bootspec) in &generation.spec.bootspec.specialisations {
                let specialised_generation = generation.specialise(name, bootspec)?;

                build_generation_artifacts(self, &specialised_generation, generation_artifacts)
                    .context("Failed to build generation artifacts for specialisation.")?;
            }
        }

        Ok(())
    }

    /// Build the unsigned generation artifacts for a single generation.
    ///
    /// Stores the mapping from source to destination for the artifacts in the provided
    /// `GenerationArtifacts`. Does not install any files to the ESP.
    ///
    /// Because this function already has an complete view of all required paths in the ESP for
    /// this generation, it stores all paths as GC roots.
    fn build_unsigned_generation_artifacts(
        &mut self,
        generation: &Generation,
        generation_artifacts: &mut GenerationArtifacts,
    ) -> Result<()> {
        let tempdir = &generation_artifacts.tempdir;

        let bootspec = &generation.spec.bootspec.bootspec;

        let esp_gen_paths = EspGenerationPaths::new(&self.esp_paths, generation)?;
        self.gc_roots.extend(esp_gen_paths.to_iter());

        let initrd_content = fs::read(
            bootspec
                .initrd
                .as_ref()
                .context("Lanzaboote does not support missing initrd yet")?,
        )?;
        let initrd_location = tempdir
            .write_secure_file(initrd_content)
            .context("Failed to copy initrd to tempfile.")?;
        if let Some(initrd_secrets_script) = &bootspec.initrd_secrets {
            append_initrd_secrets(initrd_secrets_script, &initrd_location)?;
        }

        // The initrd and kernel don't need to be signed. The stub has their hashes embedded and
        // will refuse loading on hash mismatches.
        //
        // The kernel is not signed because systemd-boot could be tricked into loading the signed
        // kernel in combination with an malicious unsigned initrd. This could be achieved because
        // systemd-boot also honors the type #1 boot loader specification.
        generation_artifacts.add_unsigned(&bootspec.kernel, &esp_gen_paths.kernel);
        generation_artifacts.add_unsigned(&initrd_location, &esp_gen_paths.initrd);

        Ok(())
    }

    /// Build the signed generation artifacts for a single generation.
    ///
    /// Stores the mapping from source to destination for the artifacts in the provided
    /// `GenerationArtifacts`. Does not install any files to the ESP.
    ///
    /// This function expects an already pre-populated `GenerationArtifacts`. It can only be called
    /// if ALL unsigned artifacts are already built and stored in `GenerationArtifacts`. More
    /// specifically, this function can only be called after `build_unsigned_generation_artifacts`
    /// has been executed.
    fn build_signed_generation_artifacts(
        &mut self,
        generation: &Generation,
        generation_artifacts: &mut GenerationArtifacts,
    ) -> Result<()> {
        let tempdir = &generation_artifacts.tempdir;

        let bootspec = &generation.spec.bootspec.bootspec;

        let esp_gen_paths = EspGenerationPaths::new(&self.esp_paths, generation)?;

        let kernel_cmdline =
            assemble_kernel_cmdline(&bootspec.init, bootspec.kernel_params.clone());

        let os_release = OsRelease::from_generation(generation)
            .context("Failed to build OsRelease from generation.")?;
        let os_release_path = tempdir
            .write_secure_file(os_release.to_string().as_bytes())
            .context("Failed to write os-release file.")?;

        let kernel_path: &Path = generation_artifacts
            .files
            .get(&esp_gen_paths.kernel)
            .context("Failed to retrieve kernel path from GenerationArtifacts.")?
            .into();

        let initrd_path = generation_artifacts
            .files
            .get(&esp_gen_paths.initrd)
            .context("Failed to retrieve initrd path from GenerationArtifacts.")?
            .into();

        let lanzaboote_image = pe::lanzaboote_image(
            tempdir,
            &self.lanzaboote_stub,
            &os_release_path,
            &kernel_cmdline,
            kernel_path,
            initrd_path,
            &esp_gen_paths,
            &self.esp_paths.esp,
        )
        .context("Failed to assemble lanzaboote image.")?;

        generation_artifacts.add_signed(&lanzaboote_image, &esp_gen_paths.lanzaboote_image);

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
            let newer_systemd_boot_available = newer_systemd_boot(from, to)?;
            if newer_systemd_boot_available {
                log::info!("Updating {to:?}...")
            };
            let systemd_boot_is_signed = &self.key_pair.verify(to);
            if !systemd_boot_is_signed {
                log::warn!("${to:?} is not signed. Replacing it with a signed binary...")
            };

            if newer_systemd_boot_available || !systemd_boot_is_signed {
                install_signed(&self.key_pair, from, to)
                    .with_context(|| format!("Failed to install systemd-boot binary to: {to:?}"))?;
            }
        }

        install(
            &self.systemd_boot_loader_config,
            &self.esp_paths.systemd_boot_loader_config,
        )
        .with_context(|| {
            format!(
                "Failed to install systemd-boot loader.conf to {:?}",
                &self.esp_paths.systemd_boot_loader_config
            )
        })?;

        Ok(())
    }
}

/// A location in the ESP together with information whether the file
/// needs to be signed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileSource {
    SignedFile(PathBuf),
    UnsignedFile(PathBuf),
}

impl<'a> From<&'a FileSource> for &'a Path {
    fn from(value: &'a FileSource) -> Self {
        match value {
            FileSource::SignedFile(p) | FileSource::UnsignedFile(p) => p,
        }
    }
}

/// Stores the source and destination of all artifacts needed to install all generations.
///
/// The key feature of this data structure is that the mappings are automatically deduplicated
/// because they are stored in a HashMap using the destination as the key. Thus, there is only
/// unique destination paths.
///
/// This enables a two step installation process where all artifacts across all generations are
/// first collected and then installed. This deduplication in the collection phase reduces the
/// number of accesesses and writes to the ESP. More importantly, however, in the second step, all
/// paths on the ESP are uniquely determined and the images can be generated while being sure that
/// the hashes embedded in them will point to a valid file on the ESP because the file will not be
/// overwritten by a later generation.
struct GenerationArtifacts {
    /// Temporary directory that stores all temporary files that are created when building the
    /// GenerationArtifacts.
    tempdir: TempDir,

    /// A mapping from target location to source.
    files: BTreeMap<PathBuf, FileSource>,
}

impl GenerationArtifacts {
    fn new() -> Result<Self> {
        Ok(Self {
            tempdir: TempDir::new().context("Failed to create temporary directory.")?,
            files: Default::default(),
        })
    }

    /// Add a file to be installed.
    ///
    /// Adding the same file multiple times with the same source is ok
    /// and will drop the old source.
    fn add_file(&mut self, from: FileSource, to: &Path) {
        if let Some(_prev_from) = self.files.insert(to.to_path_buf(), from) {
            // Should we log something here?
        }
    }

    /// Add source and destination of a PE file to be signed.
    ///
    /// Files are stored in the HashMap using their destination path as the key to ensure that the
    /// destination paths are unique.
    fn add_signed(&mut self, from: &Path, to: &Path) {
        self.add_file(FileSource::SignedFile(from.to_path_buf()), to);
    }

    /// Add source and destination of an arbitrary file.
    fn add_unsigned(&mut self, from: &Path, to: &Path) {
        self.add_file(FileSource::UnsignedFile(from.to_path_buf()), to);
    }

    /// Install all files to the ESP.
    fn install(&self, key_pair: &KeyPair) -> Result<()> {
        for (to, from) in &self.files {
            match from {
                FileSource::SignedFile(from) => {
                    install_signed(key_pair, from, to).with_context(|| {
                        format!("Failed to sign and install from {from:?} to {to:?}")
                    })?
                }
                FileSource::UnsignedFile(from) => install(from, to)
                    .with_context(|| format!("Failed to install from {from:?} to {to:?}"))?,
            }
        }

        Ok(())
    }
}

/// Install a PE file. The PE gets signed in the process.
///
/// If the file already exists at the destination, it is overwritten.
///
/// This is implemented as an atomic write. The file is first written to the destination with a
/// `.tmp` suffix and then renamed to its final name. This is atomic, because a rename is an atomic
/// operation on POSIX platforms.
fn install_signed(key_pair: &KeyPair, from: &Path, to: &Path) -> Result<()> {
    log::debug!("Signing and installing {to:?}...");
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
/// The file is only copied if
///     (1) it doesn't exist at the destination or,
///     (2) the hash of the file at the destination does not match the hash of the source file.
fn install(from: &Path, to: &Path) -> Result<()> {
    if !to.exists() || file_hash(from)? != file_hash(to)? {
        force_install(from, to)?;
    }
    Ok(())
}

/// Forcibly install an arbitrary file.
///
/// If the file already exists at the destination, it is overwritten.
///
/// This function is only designed to copy files to the ESP. It sets the permission bits of the
/// file at the destination to 0o755, the expected permissions for a vfat ESP. This is useful for
/// producing file systems trees which can then be converted to a file system image.
fn force_install(from: &Path, to: &Path) -> Result<()> {
    log::debug!("Installing {to:?}...");
    ensure_parent_dir(to);
    atomic_copy(from, to)?;
    set_permission_bits(to, 0o755)
        .with_context(|| format!("Failed to set permission bits to 0o755 on file: {to:?}"))?;
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
