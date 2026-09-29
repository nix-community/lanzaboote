use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tempfile::tempdir;

use crate::common::{self, count_files};

#[test]
fn keep_only_configured_number_of_generations() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let stub_count = || count_files(&esp_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp_mountpoint.path().join("EFI/nixos")).unwrap();

    // Install all 3 generations.
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());
    assert_eq!(stub_count(), 6, "Wrong number of stubs after installation");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after installation"
    );

    // Call `lanzatool install` again with a config limit of 2 and assert that one is deleted.
    // In addition, the garbage kernel should be deleted as well.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());
    assert_eq!(stub_count(), 4, "Wrong number of stubs after gc.");
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
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let stub_count = || count_files(&esp_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp_mountpoint.path().join("EFI/nixos")).unwrap();

    // Install all 3 generations.
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());

    // Create a garbage kernel, which should be deleted.
    fs::write(
        esp_mountpoint.path().join("EFI/nixos/kernel-garbage.efi"),
        "garbage",
    )?;

    // Call `lanzatool install` again with a config limit of 2.
    // In addition, the garbage kernel should be deleted as well.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());

    assert_eq!(stub_count(), 4, "Wrong number of stubs after gc.");
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
    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());

    let unrelated_loader_config = esp_mountpoint.path().join("loader/loader.conf");
    let unrelated_uki = esp_mountpoint.path().join("EFI/Linux/ubuntu.efi");
    let unrelated_os = esp_mountpoint.path().join("EFI/windows");
    let unrelated_firmware = esp_mountpoint.path().join("dell");
    fs::File::create(&unrelated_loader_config)?;
    fs::File::create(&unrelated_uki)?;
    fs::create_dir(&unrelated_os)?;
    fs::create_dir(&unrelated_firmware)?;

    // Call `lanzatool install` again with a config limit of 2.
    let output1 = common::lanzaboote_install(2, esp_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());

    assert!(unrelated_loader_config.exists());
    assert!(unrelated_uki.exists());
    assert!(unrelated_os.exists());
    assert!(unrelated_firmware.exists());

    Ok(())
}

fn find_file_with_prefix(path: impl AsRef<Path>, prefix: &str) {
    let found = fs::read_dir(path)
        .expect("Directory doesn't exist")
        .find(|entry| {
            entry.as_ref().is_ok_and(|e| {
                e.path()
                    .file_name()
                    .is_some_and(|filename| filename.to_string_lossy().starts_with(prefix))
            })
        });
    assert!(found.is_some_and(|e| e.is_ok()));
}

#[test]
fn keep_protected_boot() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let mut generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let stub_count = || count_files(&esp_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp_mountpoint.path().join("EFI/nixos")).unwrap();

    // Install all 3 generations.
    let output0 =
        common::Lanzaboote::new().install(esp_mountpoint.path(), generation_links.clone())?;
    assert!(output0.status.success());
    assert_eq!(stub_count(), 6, "Wrong number of stubs after installation");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after installation"
    );

    let uki_dir = esp_mountpoint.path().join("EFI/Linux");
    find_file_with_prefix(&uki_dir, "nixos-generation-1");
    find_file_with_prefix(&uki_dir, "nixos-generation-2");
    find_file_with_prefix(&uki_dir, "nixos-generation-3");

    // Add a new generation
    generation_links.push(
        common::setup_generation_link(tmpdir.path(), profiles.path(), 4)
            .expect("Failed to setup generation link"),
    );

    // Call `lanzatool install` again with a config limit of 2 and assert that one is deleted.
    // In addition, the garbage kernel should be deleted as well.
    let output1 = common::Lanzaboote::new()
        .config_limit(3)
        .protected_system(generation_links[0].clone())
        .install(esp_mountpoint.path(), generation_links)?;
    assert!(output1.status.success());
    assert_eq!(stub_count(), 6, "Wrong number of stubs after gc.");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after gc."
    );

    let uki_dir = esp_mountpoint.path().join("EFI/Linux");
    // Note how 1 is preserved because it's the booted version
    find_file_with_prefix(&uki_dir, "nixos-generation-1");
    // Note how 2 is missing here
    find_file_with_prefix(&uki_dir, "nixos-generation-3");
    find_file_with_prefix(&uki_dir, "nixos-generation-4");

    Ok(())
}
