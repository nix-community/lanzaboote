use std::fs;

use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::bootspec::Bootspec;
use crate::esp::EspPaths;
use crate::pe;

pub fn install(
    _: &Path,
    bootspec: &Path,
    lanzaboote_stub: &Path,
    initrd_stub: &Path,
) -> Result<()> {
    let bootspec_doc: Bootspec = serde_json::from_slice(&fs::read(bootspec)?)?;

    let esp_paths = EspPaths::new(&bootspec_doc.v1.extension.esp);

    let lanzaboote_image = pe::assemble_image(
        lanzaboote_stub,
        &bootspec_doc.v1.extension.os_release,
        &bootspec_doc.v1.kernel_params,
        &esp_paths.kernel,
        &esp_paths.initrd,
    )
    .expect("Failed to assemble stub");

    let wrapped_initrd =
        pe::wrap_initrd(initrd_stub, &bootspec_doc.v1.initrd).expect("Failed to assemble stub");

    // Copy the files to the ESP
    fs::create_dir_all(&esp_paths.nixos)?;
    fs::copy(bootspec_doc.v1.kernel, esp_paths.kernel)?;
    fs::copy(wrapped_initrd, esp_paths.initrd)?;

    fs::create_dir_all(&esp_paths.linux)?;
    fs::copy(lanzaboote_image, esp_paths.lanzaboote_image)?;
    // install_systemd_boot(bootctl, &esp)?;

    Ok(())
}

fn _install_systemd_boot(bootctl: &Path, esp: &Path) -> Result<()> {
    let args = vec![
        String::from("install"),
        String::from("--path"),
        esp.display().to_string(),
    ];

    let status = Command::new(&bootctl).args(&args).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed success run `{}` with args `{:?}`",
            &bootctl.display(),
            &args
        )
        .into());
    }
    Ok(())
}
