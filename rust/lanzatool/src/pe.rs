use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use goblin::pe::PE;

use tempfile::TempDir;

/// Attach all information that lanzaboote needs into the PE binary.
///
/// When this function is called the referenced files already need to
/// be present in the ESP. This is required, because we need to read
/// them to compute hashes.
pub fn lanzaboote_image(
    target_dir: &TempDir,
    lanzaboote_stub: &Path,
    os_release: &Path,
    kernel_cmdline: &[String],
    kernel_path: &Path,
    initrd_path: &Path,
    esp: &Path,
) -> Result<PathBuf> {
    // objcopy can only copy files into the PE binary. That's why we
    // have to write the contents of some bootspec properties to disk.
    let kernel_cmdline_file = write_to_tmp(target_dir, "kernel-cmdline", kernel_cmdline.join(" "))?;

    let kernel_path_file = write_to_tmp(
        target_dir,
        "kernel-esp-path",
        esp_relative_uefi_path(esp, kernel_path)?,
    )?;
    let kernel_hash_file = write_to_tmp(
        target_dir,
        "kernel-hash",
        file_hash(kernel_path)?.as_bytes(),
    )?;

    let initrd_path_file = write_to_tmp(
        target_dir,
        "initrd-esp-path",
        esp_relative_uefi_path(esp, initrd_path)?,
    )?;
    let initrd_hash_file = write_to_tmp(
        target_dir,
        "initrd-hash",
        file_hash(initrd_path)?.as_bytes(),
    )?;

    let os_release_offs = stub_offset(lanzaboote_stub)?;
    let kernel_cmdline_offs = os_release_offs + file_size(os_release)?;
    let initrd_path_offs = kernel_cmdline_offs + file_size(&kernel_cmdline_file)?;
    let kernel_path_offs = initrd_path_offs + file_size(&initrd_path_file)?;
    let initrd_hash_offs = kernel_path_offs + file_size(&kernel_path_file)?;
    let kernel_hash_offs = initrd_hash_offs + file_size(&initrd_hash_file)?;

    let sections = vec![
        s(".osrel", os_release, os_release_offs),
        s(".cmdline", kernel_cmdline_file, kernel_cmdline_offs),
        s(".initrdp", initrd_path_file, initrd_path_offs),
        s(".kernelp", kernel_path_file, kernel_path_offs),
        s(".initrdh", initrd_hash_file, initrd_hash_offs),
        s(".kernelh", kernel_hash_file, kernel_hash_offs),
    ];

    wrap_in_pe(target_dir, "lanzaboote-stub.efi", lanzaboote_stub, sections)
}

/// Compute the blake3 hash of a file.
fn file_hash(file: &Path) -> Result<blake3::Hash> {
    Ok(blake3::hash(&fs::read(file)?))
}

/// Take a PE binary stub and attach sections to it.
///
/// The result is then written to a new file. Returns the filename of
/// the generated file.
fn wrap_in_pe(
    target_dir: &TempDir,
    output_filename: &str,
    stub: &Path,
    sections: Vec<Section>,
) -> Result<PathBuf> {
    let image_path = target_dir.path().join(output_filename);
    let _ = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)
        .open(&image_path)
        .context("Failed to generate named temp file")?;

    let mut args: Vec<OsString> = sections.iter().flat_map(Section::to_objcopy).collect();

    [stub.as_os_str(), image_path.as_os_str()]
        .iter()
        .for_each(|a| args.push(a.into()));

    let status = Command::new("objcopy")
        .args(&args)
        .status()
        .context("Failed to run objcopy command")?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to wrap in pe with args `{:?}`",
            &args
        ));
    }

    Ok(image_path)
}

struct Section {
    name: &'static str,
    file_path: PathBuf,
    offset: u64,
}

impl Section {
    /// Create objcopy `-add-section` command line parameters that
    /// attach the section to a PE file.
    fn to_objcopy(&self) -> Vec<OsString> {
        // There is unfortunately no format! for OsString, so we cannot
        // just format a path.
        let mut map_str: OsString = format!("{}=", self.name).into();
        map_str.push(&self.file_path);

        vec![
            OsString::from("--add-section"),
            map_str,
            OsString::from("--change-section-vma"),
            format!("{}={:#x}", self.name, self.offset).into(),
        ]
    }
}

fn s(name: &'static str, file_path: impl AsRef<Path>, offset: u64) -> Section {
    Section {
        name,
        file_path: file_path.as_ref().into(),
        offset,
    }
}

/// Write a `u8` slice to a temporary file.
fn write_to_tmp(
    secure_temp: &TempDir,
    filename: &str,
    contents: impl AsRef<[u8]>,
) -> Result<PathBuf> {
    let path = secure_temp.path().join(filename);

    let mut tmpfile = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .context("Failed to create tempfile")?;

    tmpfile
        .write_all(contents.as_ref())
        .context("Failed to write to tempfile")?;

    Ok(path)
}

/// Convert a path to an UEFI path relative to the specified ESP.
fn esp_relative_uefi_path(esp: &Path, path: &Path) -> Result<String> {
    let relative_path = path
        .strip_prefix(esp)
        .with_context(|| format!("Failed to strip esp prefix: {:?} from: {:?}", esp, path))?;
    let uefi_path = uefi_path(relative_path)?;
    Ok(format!("\\{}", &uefi_path))
}

/// Convert a path to a UEFI string representation.
///
/// This might not _necessarily_ produce a valid UEFI path, since some UEFI implementations might
/// not support UTF-8 strings. A Rust String, however, is _always_ valid UTF-8.
fn uefi_path(path: &Path) -> Result<String> {
    path.to_str()
        .to_owned()
        .map(|x| x.replace('/', "\\"))
        .with_context(|| format!("Failed to convert {:?} to an UEFI path", path))
}

fn stub_offset(binary: &Path) -> Result<u64> {
    let pe_binary = fs::read(binary).context("Failed to read PE binary file")?;
    let pe = PE::parse(&pe_binary).context("Failed to parse PE binary file")?;

    let image_base = image_base(&pe);

    // The Virtual Memory Addresss (VMA) is relative to the image base, aka the image base
    // needs to be added to the virtual address to get the actual (but still virtual address)
    Ok(u64::from(
        pe.sections
            .last()
            .map(|s| s.virtual_size + s.virtual_address)
            .expect("Failed to calculate offset"),
    ) + image_base)
}

fn image_base(pe: &PE) -> u64 {
    pe.header
        .optional_header
        .expect("Failed to find optional header, you're fucked")
        .windows_fields
        .image_base
}

fn file_size(path: impl AsRef<Path>) -> Result<u64> {
    Ok(fs::metadata(&path)
        .with_context(|| {
            format!(
                "Failed to read file metadata to calculate its size: {:?}",
                path.as_ref()
            )
        })?
        .size())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_to_valid_uefi_path_relative_to_esp() {
        let esp = Path::new("esp");
        let path = Path::new("esp/lanzaboote/is/great.txt");
        let converted_path = esp_relative_uefi_path(esp, path).unwrap();
        let expected_path = String::from("\\lanzaboote\\is\\great.txt");
        assert_eq!(converted_path, expected_path);
    }

    #[test]
    fn convert_to_valid_uefi_path() {
        let path = Path::new("lanzaboote/is/great.txt");
        let converted_path = uefi_path(path).unwrap();
        let expected_path = String::from("lanzaboote\\is\\great.txt");
        assert_eq!(converted_path, expected_path);
    }
}
