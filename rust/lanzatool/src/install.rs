use std::fs;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use nix::unistd::sync;

use anyhow::{Context, Result};

use crate::bootspec::Bootspec;
use crate::esp::EspPaths;
use crate::pe;
use crate::signer::Signer;

use tempfile::tempdir;

use std::process::Command;

pub fn install(
    public_key: &Path,
    private_key: &Path,
    pki_bundle: &Option<PathBuf>,
    auto_enroll: bool,
    bootspec: &Path,
    generations: Vec<PathBuf>,
    lanzaboote_stub: &Path,
    initrd_stub: &Path,
) -> Result<()> {
    for generation in generations {
        let generation_version = extract_generation_version(&generation).with_context(|| {
            format!(
                "Failed to extract generation version from generation: {}",
                generation.display()
            )
        })?;

        println!("Installing generation {generation_version}");

        println!("Reading main bootspec...");
        let top_bootspec_doc: Bootspec =
            serde_json::from_slice(&fs::read(bootspec).context("Failed to read main bootspec file")?)
                .context("Failed to parse main bootspec json")?;

        let specialisations = top_bootspec_doc.specialisation.clone();

        install_generation(
            generation_version,
            public_key,
            private_key,
            pki_bundle,
            auto_enroll,
            top_bootspec_doc,
            lanzaboote_stub,
            initrd_stub,
            None
        )?;

        println!("Installing specialisations (generation {generation_version})...");
        specialisations.into_iter().for_each(move |(variant_name, spec_bootspec_doc)| {
            println!("\tInstalling specialisation {variant_name}");

            // TODO: chain the results.
            install_generation(
                generation_version,
                public_key,
                private_key,
                pki_bundle,
                auto_enroll,
                spec_bootspec_doc,
                lanzaboote_stub,
                initrd_stub,
                Some(variant_name)
            ).unwrap()
        });
    }

    Ok(())
}

fn extract_generation_version(path: impl AsRef<Path>) -> Result<u64> {
    let file_name = path.as_ref().file_name().ok_or(anyhow::anyhow!(
        "Failed to extract file name from generation"
    ))?;
    let file_name_str = file_name
        .to_str()
        .with_context(|| "Failed to convert file name of generation to string")?;

    let generation_version = file_name_str
        .split("-")
        .nth(1)
        .ok_or(anyhow::anyhow!("Failed to extract version from generation"))?;

    Ok(generation_version
        .parse()
        .with_context(|| format!("Failed to parse generation version: {}", generation_version))?)
}

fn install_generation(
    generation: u64,
    public_key: &Path,
    private_key: &Path,
    _pki_bundle: &Option<PathBuf>,
    _auto_enroll: bool,
    bootspec_doc: Bootspec,
    lanzaboote_stub: &Path,
    initrd_stub: &Path,
    specialisation_name: Option<String>
) -> Result<()> {
    let esp_paths = EspPaths::new(&bootspec_doc.extension.esp, generation, &bootspec_doc, specialisation_name)?;

    println!("Assembling lanzaboote image...");

    let kernel_cmdline = assemble_kernel_cmdline(bootspec_doc.init, bootspec_doc.kernel_params);

    // prepare a secure temporary directory
    // permission bits are not set, because when files below
    // are opened, they are opened with 600 mode bits.
    // hence, they cannot be read except by the current user
    // which is assumed to be root in most cases.
    // TODO(Raito): prove to niksnur this is actually acceptable.
    let secure_temp_dir = tempdir()?;

    let lanzaboote_image = pe::lanzaboote_image(
        &secure_temp_dir,
        lanzaboote_stub,
        &bootspec_doc.extension.os_release,
        &kernel_cmdline,
        &esp_paths.kernel,
        &esp_paths.initrd,
        &esp_paths.esp,
    )
    .context("Failed to assemble stub")?;

    println!("Wrapping initrd into a PE binary...");

    let initrd_location = secure_temp_dir.path().join("initrd");
    copy(&bootspec_doc.initrd, &initrd_location)?;
    if let Some(initrd_secrets_script) = bootspec_doc.initrd_secrets {
        append_initrd_secrets(&initrd_secrets_script,
            &initrd_location)?;
    }
    let wrapped_initrd =
        pe::wrap_initrd(&secure_temp_dir, initrd_stub, &initrd_location).context("Failed to assemble stub")?;

    println!("Copy files to EFI system partition...");

    let systemd_boot = bootspec_doc
        .extension
        .systemd
        .join("lib/systemd/boot/efi/systemd-bootx64.efi");

    let files_to_copy = [
        (bootspec_doc.kernel, &esp_paths.kernel),
        (wrapped_initrd, &esp_paths.initrd),
        (lanzaboote_image, &esp_paths.lanzaboote_image),
        (systemd_boot.clone(), &esp_paths.efi_fallback),
        (systemd_boot, &esp_paths.systemd_boot),
    ];

    for (source, target) in files_to_copy {
        copy(&source, &target)?;
    }

    // TODO: we should implement sign_and_copy which would be secure
    // by construction for TOCTOU.

    println!("Signing files...");

    let signer = Signer::new(&public_key, &private_key);

    let files_to_sign = [
        &esp_paths.efi_fallback,
        &esp_paths.systemd_boot,
        &esp_paths.lanzaboote_image,
        &esp_paths.kernel,
        &esp_paths.initrd,
    ];

    for file in files_to_sign {
        println!("Signing {}...", file.display());
        signer
            .sign_file(&file)
            .with_context(|| format!("Failed to sign file {}", &file.display()))?;
        sync();
    }

    println!(
        "Successfully installed lanzaboote to '{}'",
        esp_paths.esp.display()
    );

    Ok(())
}

pub fn append_initrd_secrets(append_initrd_secrets_path: &Path, initrd_path: &PathBuf) -> Result<()> {
    let status = Command::new(append_initrd_secrets_path)
        .args(vec![
            initrd_path
        ])
        .status()
        .context("Failed to append initrd secrets")?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to append initrd secrets with args `{:?}`", vec![append_initrd_secrets_path, initrd_path]).into());
    }

    Ok(())
}

fn assemble_kernel_cmdline(init: PathBuf, kernel_params: Vec<String>) -> Vec<String> {
    let init_string = init
        .into_os_string()
        .into_string()
        .expect("Failed to convert init path to string");
    let mut kernel_cmdline: Vec<String> = vec![format!("init={}", init_string)];
    kernel_cmdline.extend(kernel_params);
    kernel_cmdline
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    match to.parent() {
        Some(parent) => fs::create_dir_all(parent).unwrap_or(()),
        _ => (),
    };
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
