use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use goblin::pe::PE;

pub fn assemble_image(
    lanzaboote_stub: &Path,
    os_release: &Path,
    kernel_cmdline: &[String],
    kernel_path: &Path,
    initrd_path: &Path,
) -> Result<PathBuf> {
    // objcopy copies files into the PE binary. That's why we have to write the contents
    // of some bootspec properties to disk
    let kernel_cmdline_file = Path::new("/tmp/kernel_cmdline");
    let kernel_path_file = Path::new("/tmp/kernel_path");
    let initrd_path_file = Path::new("/tmp/initrd_path");

    fs::write(kernel_cmdline_file, kernel_cmdline.join(" "))?;
    fs::write(kernel_path_file, efi_relative_path_string(kernel_path))?;
    fs::write(initrd_path_file, efi_relative_path_string(initrd_path))?;

    let os_release_offs = stub_offset(lanzaboote_stub)?;

    let kernel_cmdline_offs = os_release_offs + file_size(os_release)?;
    let initrd_path_offs = kernel_cmdline_offs + file_size(kernel_cmdline_file)?;
    let kernel_path_offs = initrd_path_offs + file_size(initrd_path_file)?;

    let lanzaboote_image = PathBuf::from("/tmp/lanzaboote-image.efi");

    let args = vec![
        String::from("--add-section"),
        format!(".osrel={}", path_to_string(os_release)),
        String::from("--change-section-vma"),
        format!(".osrel={:#x}", os_release_offs),
        String::from("--add-section"),
        format!(".cmdline={}", path_to_string(kernel_cmdline_file)),
        String::from("--change-section-vma"),
        format!(".cmdline={:#x}", kernel_cmdline_offs),
        String::from("--add-section"),
        format!(".initrdp={}", path_to_string(initrd_path_file)),
        String::from("--change-section-vma"),
        format!(".initrdp={:#x}", initrd_path_offs),
        String::from("--add-section"),
        format!(".kernelp={}", path_to_string(kernel_path_file)),
        String::from("--change-section-vma"),
        format!(".kernelp={:#x}", kernel_path_offs),
        path_to_string(lanzaboote_stub),
        path_to_string(&lanzaboote_image),
    ];

    let status = Command::new("objcopy").args(&args).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to build stub with args `{:?}`", &args).into());
    }

    Ok(lanzaboote_image)
}

fn efi_relative_path_string(path: &Path) -> String {
    let relative_path = path
        .strip_prefix("esp")
        .expect("Failed to make path relative to esp")
        .to_owned();
    let relative_path_string = relative_path
        .into_os_string()
        .into_string()
        .expect("Failed to convert path '{}' to a relative string path")
        .replace("/", "\\");
    format!("\\{}", &relative_path_string)
}

pub fn wrap_initrd(initrd_stub: &Path, initrd: &Path) -> Result<PathBuf> {
    let initrd_offs = stub_offset(initrd_stub)?;

    let wrapped_initrd = PathBuf::from("/tmp/initrd.efi");

    let args = vec![
        String::from("--add-section"),
        format!(".initrd={}", path_to_string(initrd)),
        String::from("--change-section-vma"),
        format!(".initrd={:#x}", initrd_offs),
        path_to_string(initrd_stub),
        path_to_string(&wrapped_initrd),
    ];

    let status = Command::new("objcopy").args(&args).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to wrap initrd into a PE binary with args `{:?}`",
            &args
        )
        .into());
    }

    Ok(wrapped_initrd)
}

fn stub_offset(binary: &Path) -> Result<u64> {
    let pe_binary = fs::read(binary)?;
    let pe = PE::parse(&pe_binary)?;

    let image_base = image_base(&pe);

    // The Virtual Memory Addresss (VMA) is relative to the image base, aka the image base
    // needs to be added to the virtual address to get the actual (but still virtual address)
    Ok(u64::from(
        pe.sections
            .last()
            .and_then(|s| Some(s.virtual_size + s.virtual_address))
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

// All Linux file paths should be convertable to strings
fn path_to_string(path: &Path) -> String {
    path.to_owned()
        .into_os_string()
        .into_string()
        .expect(&format!(
            "Failed to convert path '{}' to a string",
            path.display()
        ))
}

fn file_size(path: &Path) -> Result<u64> {
    Ok(fs::File::open(path)?.metadata()?.size())
}
