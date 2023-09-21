use std::collections::BTreeSet;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::string::ToString;

use anyhow::{anyhow, Context, Result};
use lanzaboote_tool::install::{install_signed, install};
use nix::unistd::syncfs;

use crate::esp::SystemdEspPaths;
use crate::version::SystemdVersion;
use lanzaboote_tool::builder::{GenerationArtifacts, Builder};
use lanzaboote_tool::esp::EspPaths;
use lanzaboote_tool::gc::Roots;
use lanzaboote_tool::generation::{Generation, GenerationLink};
use lanzaboote_tool::signature::KeyPair;

pub struct Installer {
    broken_gens: BTreeSet<u64>,
    gc_roots: Roots,
    lanzaboote_stub: PathBuf,
    systemd: PathBuf,
    systemd_boot_loader_config: PathBuf,
    key_pair: KeyPair,
    configuration_limit: usize,
    esp_paths: SystemdEspPaths,
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
        let esp_paths = SystemdEspPaths::new(esp);
        gc_roots.extend(esp_paths.iter());

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

        // We compute the flattened tree of generations,
        // i.e. for each generation, we parse it, remove it if broken
        // Then, for each generation, we parse its specialisations and flatten them in the list,
        // remove them if broken again.
        //
        // So, if you have:
        //
        //
        //
        //            generation A                       generation B
        //           /          \
        //          /            \
        //         /              \
        //  specialization A_0  specialization A_1
        //
        //  You will see [A, A_0, A_1, B] in the `generations` variable.
        let generations = links
            .into_iter()
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
            .flat_map(|ref generation| {
                generation.spec.bootspec.specialisations.iter().filter_map(move |(name, specialisation)| {
                    let specialisation_result = generation.specialise(&name, &specialisation)
                        .with_context(|| format!("Failed to build specialisation {name} from generation: {specialisation:?}"));

                    // TODO: add broken gens here.

                    specialisation_result.ok()
                }).collect::<Vec<Generation>>()
            })
            .collect::<Vec<Generation>>();

        if generations.is_empty() {
            // We can't continue, because we would remove all boot entries, if we did.
            return Err(anyhow!("No bootable generations found! Aborting to avoid unbootable system. Please check for Lanzaboote updates!"));
        }

        let mut builders: Vec<Builder<10, SystemdEspPaths>> = Vec::with_capacity(generations.len());
        for generation in generations {
            let builder = Builder::from_existing_artifacts(&self.lanzaboote_stub, generation, &self.esp_paths)
                .context("Failed to create a builder for a generation")?;

            builders.push(builder);
        }

        for builder in &mut builders {
            builder.build_unsigned(&mut generation_artifacts).context("Failed to build unsigned generation artifacts.")?;
        }

        for builder in &mut builders {
            builder.build_signed(&mut generation_artifacts).context("Failed to build signed generation artifacts.")?;
        }

        generation_artifacts
            .install(&self.key_pair)
            .context("Failed to install files.")?;

        // Sync files to persistent storage. This may improve the
        // chance of a consistent boot directory in case the system
        // crashes.
        let boot = File::open(&self.esp_paths.esp).context("Failed to open ESP root directory.")?;
        syncfs(boot.as_raw_fd()).context("Failed to sync ESP filesystem.")?;

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
