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

/// Check whether any stub for the given generation number exists in the linux directory.
fn has_generation(linux_dir: &Path, version: u64) -> bool {
    fs::read_dir(linux_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|s| s.starts_with(&format!("nixos-generation-{version}-")))
        })
}

/// Simulate a generation becoming known-good by removing the boot counting
/// suffix from its stubs (as systemd-boot does after a successful boot).
fn mark_generation_known_good(linux_dir: &Path, version: u64, tries: u32) -> Result<()> {
    for entry in fs::read_dir(linux_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_str().unwrap().to_string();
        if name.starts_with(&format!("nixos-generation-{version}-"))
            && name.contains(&format!("+{tries}"))
        {
            let new_name = name.replace(&format!("+{tries}"), "");
            fs::rename(entry.path(), linux_dir.join(&new_name))?;
        }
    }
    Ok(())
}

/// With boot counting enabled and a configuration limit, known-good entries
/// can be pushed out by newer unassessed entries. The installer must preserve
/// at least the most recent known-good generation as a bootable fallback.
#[test]
fn preserve_known_good_entry_with_boot_counting() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = (1..=5)
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let linux_dir = esp_mountpoint.path().join("EFI/Linux");

    // Install all 5 generations with boot counting (tries=3).
    let output = common::lanzaboote_install_with_bootcounting(
        0,
        3,
        esp_mountpoint.path(),
        generation_links.clone(),
    )?;
    assert!(output.status.success());

    // Simulate generation 3 becoming known-good.
    mark_generation_known_good(&linux_dir, 3, 3)?;

    // Install with limit=2 and boot counting. Without the fix, generation 3
    // would be garbage collected, leaving only unassessed entries 4 and 5.
    let output = common::lanzaboote_install_with_bootcounting(
        2,
        3,
        esp_mountpoint.path(),
        generation_links,
    )?;
    assert!(output.status.success());

    // Generation 3 (known-good fallback) and 4, 5 (latest 2) should be present.
    assert!(
        has_generation(&linux_dir, 3),
        "Known-good generation 3 should be preserved"
    );
    assert!(
        has_generation(&linux_dir, 4),
        "Generation 4 should be present"
    );
    assert!(
        has_generation(&linux_dir, 5),
        "Generation 5 should be present"
    );

    // Older non-known-good generations should be garbage collected.
    assert!(
        !has_generation(&linux_dir, 1),
        "Generation 1 should be garbage collected"
    );
    assert!(
        !has_generation(&linux_dir, 2),
        "Generation 2 should be garbage collected"
    );

    Ok(())
}

/// When a known-good generation is already within the configuration limit,
/// no extra entry should be added.
#[test]
fn no_extra_entry_when_known_good_within_limit() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let generation_links: Vec<PathBuf> = (1..=3)
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v)
                .expect("Failed to setup generation link")
        })
        .collect();
    let linux_dir = esp_mountpoint.path().join("EFI/Linux");
    let stub_count = || count_files(&linux_dir).unwrap();

    // Install all 3 generations with boot counting.
    let output = common::lanzaboote_install_with_bootcounting(
        0,
        3,
        esp_mountpoint.path(),
        generation_links.clone(),
    )?;
    assert!(output.status.success());

    // Mark generation 3 (the latest) as known-good.
    mark_generation_known_good(&linux_dir, 3, 3)?;

    // Install with limit=2 and boot counting.
    // Generation 3 is already among the latest 2, so no extra entry is needed.
    let output = common::lanzaboote_install_with_bootcounting(
        2,
        3,
        esp_mountpoint.path(),
        generation_links,
    )?;
    assert!(output.status.success());

    // Exactly 2 generations should remain (4 stubs: 2 main + 2 specialisations).
    assert_eq!(stub_count(), 4, "Should have exactly 2 generations");
    assert!(
        has_generation(&linux_dir, 2),
        "Generation 2 should be present"
    );
    assert!(
        has_generation(&linux_dir, 3),
        "Generation 3 should be present"
    );
    assert!(
        !has_generation(&linux_dir, 1),
        "Generation 1 should be garbage collected"
    );

    Ok(())
}
