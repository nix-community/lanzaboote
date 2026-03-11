use std::fs;
use std::path::PathBuf;

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
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
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
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
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
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
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

#[test]
fn retain_newest_generations_globally_by_build_time() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let oldest_toplevel = common::setup_toplevel(tmpdir.path())?;
    let oldest = common::setup_generation_link_from_toplevel(
        &oldest_toplevel,
        profiles.path(),
        50,
        Some("old"),
    )?;
    common::set_generation_link_mtime(&oldest, 0)?;
    let middle_toplevel = common::setup_toplevel(tmpdir.path())?;
    let middle = common::setup_generation_link_from_toplevel(
        &middle_toplevel,
        profiles.path(),
        1,
        Some("middle"),
    )?;
    common::set_generation_link_mtime(&middle, 10)?;
    let newest_toplevel = common::setup_toplevel(tmpdir.path())?;
    let newest = common::setup_generation_link_from_toplevel(
        &newest_toplevel,
        profiles.path(),
        2,
        Some("newest"),
    )?;
    common::set_generation_link_mtime(&newest, 20)?;

    let output = common::lanzaboote_install(
        2,
        esp_mountpoint.path(),
        vec![oldest, middle.clone(), newest.clone()],
    )?;
    assert!(output.status.success());

    assert!(
        !common::image_path(&esp_mountpoint, 50, Some("old"), false, &oldest_toplevel)?.exists(),
        "Oldest profile generation should have been dropped by global retention",
    );
    assert!(
        common::image_path(&esp_mountpoint, 1, Some("middle"), false, &middle_toplevel)?.exists()
    );
    assert!(
        common::image_path(&esp_mountpoint, 2, Some("newest"), false, &newest_toplevel)?.exists()
    );
    assert_eq!(
        common::count_files(&esp_mountpoint.path().join("EFI/Linux"))?,
        4
    );

    Ok(())
}

#[test]
fn retain_original_generation_age_when_profile_link_is_recreated() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let old_toplevel = common::setup_toplevel(tmpdir.path())?;
    let old_default =
        common::setup_generation_link_from_toplevel(&old_toplevel, profiles.path(), 1, None)?;
    common::set_generation_link_mtime(&old_default, 0)?;

    let new_toplevel = common::setup_toplevel(tmpdir.path())?;
    let new_default =
        common::setup_generation_link_from_toplevel(&new_toplevel, profiles.path(), 2, None)?;
    common::set_generation_link_mtime(&new_default, 10)?;

    let old_profile_copy = common::setup_generation_link_from_toplevel(
        &old_toplevel,
        profiles.path(),
        1,
        Some("copied"),
    )?;
    common::set_generation_link_mtime(&old_profile_copy, 20)?;

    let output = common::lanzaboote_install(
        1,
        esp_mountpoint.path(),
        vec![old_default, new_default, old_profile_copy],
    )?;
    assert!(output.status.success());

    assert!(
        !common::image_path(&esp_mountpoint, 1, None, true, &old_toplevel)?.exists(),
        "the oldest default generation should be pruned even if copied to a newer profile link",
    );
    assert!(
        common::image_path(&esp_mountpoint, 2, None, true, &new_toplevel)?.exists(),
        "the genuinely newer generation should be retained",
    );
    assert!(
        !common::image_path(&esp_mountpoint, 1, Some("copied"), false, &old_toplevel)?.exists(),
        "a recreated profile link must not make an older generation outrank a genuinely newer one",
    );

    Ok(())
}
