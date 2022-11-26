use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use goblin::pe::PE;

use crate::utils;

use tempfile::TempDir;

pub fn lanzaboote_image(
    target_dir: &TempDir,
    lanzaboote_stub: &Path,
    os_release: &Path,
    kernel_cmdline: &[String],
    kernel_path: &Path,
    initrd_path: &Path,
    esp: &Path,
) -> Result<PathBuf> {
    // objcopy copies files into the PE binary. That's why we have to write the contents
    // of some bootspec properties to disk
    let (kernel_cmdline_file, _) =
        write_to_tmp(target_dir, "kernel-cmdline", kernel_cmdline.join(" "))?;
    let (kernel_path_file, _) = write_to_tmp(
        target_dir,
        "kernel-esp-path",
        esp_relative_path_string(esp, kernel_path),
    )?;
    let (initrd_path_file, _) = write_to_tmp(
        target_dir,
        "initrd-esp-path",
        esp_relative_path_string(esp, initrd_path),
    )?;

    let os_release_offs = stub_offset(lanzaboote_stub)?;
    let kernel_cmdline_offs = os_release_offs + file_size(os_release)?;
    let initrd_path_offs = kernel_cmdline_offs + file_size(&kernel_cmdline_file)?;
    let kernel_path_offs = initrd_path_offs + file_size(&initrd_path_file)?;

    let sections = vec![
        s(".osrel", os_release, os_release_offs),
        s(".cmdline", kernel_cmdline_file, kernel_cmdline_offs),
        s(".initrdp", initrd_path_file, initrd_path_offs),
        s(".kernelp", kernel_path_file, kernel_path_offs),
    ];

    wrap_in_pe(target_dir, "lanzaboote-stub.efi", lanzaboote_stub, sections)
}

pub fn wrap_initrd(target_dir: &TempDir, initrd_stub: &Path, initrd: &Path) -> Result<PathBuf> {
    let initrd_offs = stub_offset(initrd_stub)?;
    let sections = vec![s(".initrd", initrd, initrd_offs)];
    wrap_in_pe(target_dir, "wrapped-initrd.exe", initrd_stub, sections)
}

fn wrap_in_pe(
    target_dir: &TempDir,
    filename: &str,
    stub: &Path,
    sections: Vec<Section>,
) -> Result<PathBuf> {
    let image_path = target_dir.path().join(filename);
    let _ = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)
        .open(&image_path)
        .context("Failed to generate named temp file")?;

    let mut args: Vec<String> = sections.iter().flat_map(Section::to_objcopy).collect();
    let extra_args = vec![
        utils::path_to_string(stub),
        utils::path_to_string(&image_path),
    ];
    args.extend(extra_args);

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
    fn to_objcopy(&self) -> Vec<String> {
        vec![
            String::from("--add-section"),
            format!("{}={}", self.name, utils::path_to_string(&self.file_path)),
            String::from("--change-section-vma"),
            format!("{}={:#x}", self.name, self.offset),
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

fn write_to_tmp(
    secure_temp: &TempDir,
    filename: &str,
    contents: impl AsRef<[u8]>,
) -> Result<(PathBuf, fs::File)> {
    let mut tmpfile = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)
        .open(secure_temp.path().join(filename))
        .context("Failed to create tempfile")?;
    tmpfile
        .write_all(contents.as_ref())
        .context("Failed to write to tempfile")?;
    Ok((secure_temp.path().join(filename), tmpfile))
}

fn esp_relative_path_string(esp: &Path, path: &Path) -> String {
    let relative_path = path
        .strip_prefix(esp)
        .expect("Failed to make path relative to esp")
        .to_owned();
    let relative_path_string = relative_path
        .into_os_string()
        .into_string()
        .expect("Failed to convert path '{}' to a relative string path")
        .replace('/', "\\");
    format!("\\{}", &relative_path_string)
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
    Ok(fs::File::open(path)?.metadata()?.size())
}
