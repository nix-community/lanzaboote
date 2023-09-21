use std::{path::{PathBuf, Path}, collections::BTreeMap, fs, process::Command};

use anyhow::{Result, Context};
use crate::{generation::{GenerationLink, Generation}, esp::{EspGenerationPaths, EspPaths}, gc::Roots, utils::SecureTempDirExt, os_release::OsRelease, pe};
use tempfile::TempDir;

/// A location in the ESP together with information whether the file
/// needs to be signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
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
pub struct GenerationArtifacts {
    /// Temporary directory that stores all temporary files that are created when building the
    /// GenerationArtifacts.
    tempdir: TempDir,

    /// A mapping from target location to source.
    pub files: BTreeMap<PathBuf, FileSource>,
}

impl GenerationArtifacts {
    pub fn new() -> Result<Self> {
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
    pub fn add_signed(&mut self, from: &Path, to: &Path) {
        self.add_file(FileSource::SignedFile(from.to_path_buf()), to);
    }

    /// Add source and destination of an arbitrary file.
    pub fn add_unsigned(&mut self, from: &Path, to: &Path) {
        self.add_file(FileSource::UnsignedFile(from.to_path_buf()), to);
    }
}


pub struct Builder<'a, const N: usize, P: EspPaths<N>> {
    lanzaboote_stub: &'a PathBuf,
    pub generation: Generation,
    esp_paths: &'a P,
    esp_gen_paths: EspGenerationPaths
}

pub struct BuiltStub {
    pub(crate) gc_roots: Roots,
    pub(crate) artifacts: GenerationArtifacts
}

impl<'a, 'b: 'a, const N: usize, P: EspPaths<N>> Builder<'a, N, P> {
    pub fn from_existing_artifacts(lanzaboote_stub: &'a PathBuf,
        generation: Generation,
        esp_paths: &'a P) -> Result<Self> {
        Ok(Self {
            lanzaboote_stub,
            esp_gen_paths: EspGenerationPaths::new(esp_paths, &generation).context("Failed to build ESP paths for this generation")?,
            generation,
            esp_paths,
        })
    }

    /// Build the unsigned generation artifacts for a single generation.
    ///
    /// Stores the mapping from source to destination for the artifacts in the provided
    /// `GenerationArtifacts`. Does not install any files to the ESP.
    pub fn build_unsigned(&mut self, artifacts: &mut GenerationArtifacts) -> Result<()> {
        let tempdir = &artifacts.tempdir;
        let bootspec = &self.generation.spec.bootspec.bootspec;

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
        // This is also true for probably many other bootloaders.
        artifacts.add_unsigned(&bootspec.kernel, &self.esp_gen_paths.kernel);
        artifacts.add_unsigned(&initrd_location, &self.esp_gen_paths.initrd);

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
    /// Because this function already has an complete view of all required paths in the ESP for
    /// this generation, it stores all paths as GC roots.
    pub fn build_signed(&mut self, artifacts: &mut GenerationArtifacts) -> Result<()> {
        let tempdir = &artifacts.tempdir;
        let mut gc_roots = Roots::new();
        gc_roots.extend(self.esp_gen_paths.to_iter());

        let bootspec = &self.generation.spec.bootspec.bootspec;

        let kernel_cmdline =
            assemble_kernel_cmdline(&bootspec.init, bootspec.kernel_params.clone());

        let os_release = OsRelease::from_generation(&self.generation)
            .context("Failed to build OsRelease from generation.")?;
        let os_release_path = tempdir
            .write_secure_file(os_release.to_string().as_bytes())
            .context("Failed to write os-release file.")?;

        let kernel_path: &Path = artifacts
            .files
            .get(&self.esp_gen_paths.kernel)
            .context("Failed to retrieve kernel path from GenerationArtifacts.")?
            .into();

        let initrd_path = artifacts
            .files
            .get(&self.esp_gen_paths.initrd)
            .context("Failed to retrieve initrd path from GenerationArtifacts.")?
            .into();

        let lanzaboote_image = pe::lanzaboote_image(
            tempdir,
            &self.lanzaboote_stub,
            &os_release_path,
            &kernel_cmdline,
            kernel_path,
            initrd_path,
            &self.esp_gen_paths,
            &self.esp_paths.esp_path(),
        )
        .context("Failed to assemble lanzaboote image.")?;

        artifacts.add_signed(&lanzaboote_image, &self.esp_gen_paths.lanzaboote_image);

        Ok(())
    }
}
