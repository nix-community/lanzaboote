use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use tempfile::tempdir;

use crate::common::{self, count_files};

#[test]
fn keep_only_configured_number_of_generations() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let boot_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let stub_count = || count_files(&boot_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&boot_mountpoint.path().join("EFI/nixos")).unwrap();

    // Install all 3 generations.
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), boot_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());
    assert_eq!(stub_count(), 3, "Wrong number of stubs after installation");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after installation"
    );

    // Call `lanzatool install` again with a config limit of 2 and assert that one is deleted.
    // In addition, the garbage kernel should be deleted as well.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), boot_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());
    assert_eq!(stub_count(), 2, "Wrong number of stubs after gc.");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after gc."
    );

    Ok(())
}

#[test]
fn delete_garbage_kernel() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let boot_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let stub_count = || count_files(&boot_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&boot_mountpoint.path().join("EFI/nixos")).unwrap();

    // Install all 3 generations.
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), boot_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());

    // Create a garbage kernel, which should be deleted.
    fs::write(
        boot_mountpoint.path().join("EFI/nixos/kernel-garbage.efi"),
        "garbage",
    )?;

    // Call `lanzatool install` again with a config limit of 2.
    // In addition, the garbage kernel should be deleted as well.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), boot_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());

    assert_eq!(stub_count(), 2, "Wrong number of stubs after gc.");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after gc."
    );

    Ok(())
}

#[test]
fn keep_unrelated_files_on_esp() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let boot_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();

    // Install all 3 generations.
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), boot_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());

    let unrelated_loader_config = esp_mountpoint.path().join("loader/loader.conf");
    let unrelated_uki = boot_mountpoint.path().join("EFI/Linux/ubuntu.efi");
    let unrelated_os = esp_mountpoint.path().join("EFI/windows");
    let unrelated_firmware = esp_mountpoint.path().join("dell");
    fs::File::create(&unrelated_loader_config)?;
    fs::File::create(&unrelated_uki)?;
    fs::create_dir(&unrelated_os)?;
    fs::create_dir(&unrelated_firmware)?;

    // Call `lanzatool install` again with a config limit of 2.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), boot_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());

    assert!(unrelated_loader_config.exists());
    assert!(unrelated_uki.exists());
    assert!(unrelated_os.exists());
    assert!(unrelated_firmware.exists());

    Ok(())
}
