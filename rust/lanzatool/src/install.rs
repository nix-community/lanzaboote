use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::bootspec::Bootspec;
use crate::esp::EspPaths;
use crate::pe;

pub fn install(
    _public_key: &Path,
    _private_key: &Path,
    _pki_bundle: &Path,
    _auto_enroll: bool,
    bootspec: &Path,
    lanzaboote_stub: &Path,
    initrd_stub: &Path,
) -> Result<()> {
    println!("Reading bootspec...");

    let bootspec_doc: Bootspec =
        serde_json::from_slice(&fs::read(bootspec).context("Failed to read bootspec file")?)
            .context("Failed to parse bootspec json")?;

    let esp_paths = EspPaths::new(&bootspec_doc.extension.esp);

    println!("Assembling lanzaboote image...");
    let mut kernel_cmdline: Vec<String> = vec![bootspec_doc
        .init
        .into_os_string()
        .into_string()
        .expect("Failed to convert init to string")];
    kernel_cmdline.extend(bootspec_doc.kernel_params);

    let lanzaboote_image = pe::assemble_image(
        lanzaboote_stub,
        &bootspec_doc.extension.os_release,
        &kernel_cmdline,
        &esp_paths.kernel,
        &esp_paths.initrd,
    )
    .context("Failed to assemble stub")?;

    println!("Wrapping initrd into a PE binary...");

    let wrapped_initrd =
        pe::wrap_initrd(initrd_stub, &bootspec_doc.initrd).context("Failed to assemble stub")?;

    println!("Copy files to EFI system partition...");

    let systemd_boot = bootspec_doc
        .extension
        .systemd
        .join("lib/systemd/boot/efi/systemd-bootx64.efi");

    let files_to_copy = [
        (bootspec_doc.kernel, esp_paths.kernel),
        (wrapped_initrd, esp_paths.initrd),
        (lanzaboote_image, esp_paths.lanzaboote_image),
        (systemd_boot.clone(), esp_paths.efi_fallback),
        (systemd_boot, esp_paths.systemd_boot),
    ];

    for (source, target) in files_to_copy {
        copy(&source, &target)?;
    }

    println!(
        "Succesfully installed lanzaboote to '{}'",
        esp_paths.esp.display()
    );
    Ok(())
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    match to.parent() {
        Some(parent) => fs::create_dir_all(parent).unwrap_or(()),
        _ => (),
    };
    fs::copy(from, to)
        .with_context(|| format!("Failed to copy from {} to {}", from.display(), to.display()))?;
    Ok(())
}
