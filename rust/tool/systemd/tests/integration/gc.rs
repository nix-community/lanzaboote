use std::path::PathBuf;
use std::{fs, usize};

use anyhow::Result;
use tempfile::tempdir;

use crate::common::{self, count_files};

#[test]
fn keep_only_configured_number_of_generations() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let stub_count = || count_files(&esp_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp_mountpoint.path().join("EFI/nixos")).unwrap();

    let kp_configured_num_gen =
        |gen_lnks: Vec<PathBuf>, n_gens: usize, config_lim: usize| -> Result<()> {
            // Install all generations.
            let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), gen_lnks.clone())?;
            assert!(output0.status.success());
            assert_eq!(
                stub_count(),
                2 * n_gens,
                "Wrong number of stubs after installation"
            );
            assert_eq!(
                kernel_and_initrd_count(),
                2,
                "Wrong number of kernels & initrds after installation"
            );

            // Call `lanzatool install` again with a config limit and assert that the rest are deleted.
            // In addition, the garbage kernel should be deleted as well.
            let output1 = common::lanzaboote_install(
                config_lim.try_into().unwrap(),
                esp_mountpoint.path(),
                gen_lnks,
            )?;
            assert!(output1.status.success());
            assert_eq!(stub_count(), 2 * config_lim, "Wrong number of stubs after gc.");
            assert_eq!(
                kernel_and_initrd_count(),
                2,
                "Wrong number of kernels & initrds after gc."
            );

            Ok(())
        };

    // Without profile
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
                .expect("Failed to setup generation link")
        })
        .collect();
    let _ = kp_configured_num_gen(generation_links, usize::from(3u8), usize::from(2u8));

    // With profile
    let generation_links_prof: Vec<PathBuf> = [
        ("My Prof 1", 1),
        ("My Prof 1", 2),
        ("My-Prof-2", 1),
        ("My-Prof-2", 2),
        ("My_Prof_3", 1),
        ("My_Prof_3", 2),
    ]
    .into_iter()
    .map(|(p, v)| {
        common::setup_generation_link(tmpdir.path(), profiles.path(), v, Some(p.to_string()))
            .expect("Failed to setup generation link")
    })
    .collect();
    let _ = kp_configured_num_gen(generation_links_prof, usize::from(6u8), usize::from(4u8));

    Ok(())
}

#[test]
fn delete_garbage_kernel() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let stub_count = || count_files(&esp_mountpoint.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp_mountpoint.path().join("EFI/nixos")).unwrap();

    let del_garb_kern = |gen_lnks: Vec<PathBuf>, config_lim: usize| -> Result<()> {
        // Install all generations.
        let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), gen_lnks.clone())?;
        assert!(output0.status.success());

        // Create a garbage kernel, which should be deleted.
        fs::write(
            esp_mountpoint.path().join("EFI/nixos/kernel-garbage.efi"),
            "garbage",
        )?;

        // Call `lanzatool install` again with a config limit.
        // In addition, the garbage kernel should be deleted as well.
        let output1 = common::lanzaboote_install(
            config_lim.try_into().unwrap(),
            esp_mountpoint.path(),
            gen_lnks,
        )?;
        assert!(output1.status.success());

        assert_eq!(stub_count(), 2 * config_lim, "Wrong number of stubs after gc.");
        assert_eq!(
            kernel_and_initrd_count(),
            2,
            "Wrong number of kernels & initrds after gc."
        );

        Ok(())
    };

    // Without profile
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
                .expect("Failed to setup generation link")
        })
        .collect();
    let _ = del_garb_kern(generation_links, usize::from(2u8));

    // With profile
    let generation_links_prof: Vec<PathBuf> = [
        ("My Prof 1", 1),
        ("My Prof 1", 2),
        ("My-Prof-2", 1),
        ("My-Prof-2", 2),
        ("My_Prof_3", 1),
        ("My_Prof_3", 2),
    ]
    .into_iter()
    .map(|(p, v)| {
        common::setup_generation_link(tmpdir.path(), profiles.path(), v, Some(p.to_string()))
            .expect("Failed to setup generation link")
    })
    .collect();
    let _ = del_garb_kern(generation_links_prof, usize::from(4u8));

    Ok(())
}

#[test]
fn keep_unrelated_files_on_esp() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let kp_unrel_files = |gen_lnks: Vec<PathBuf>, config_lim: usize| -> Result<()> {
        // Install all generations.
        let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), gen_lnks.clone())?;
        assert!(output0.status.success());

        let unrelated_loader_config = esp_mountpoint.path().join("loader/loader.conf");
        let unrelated_uki = esp_mountpoint.path().join("EFI/Linux/ubuntu.efi");
        let unrelated_os = esp_mountpoint.path().join("EFI/windows");
        let unrelated_firmware = esp_mountpoint.path().join("dell");
        fs::File::create(&unrelated_loader_config)?;
        fs::File::create(&unrelated_uki)?;
        fs::create_dir(&unrelated_os)?;
        fs::create_dir(&unrelated_firmware)?;

        // Call `lanzatool install` again with a config limit.
        let output1 = common::lanzaboote_install(
            config_lim.try_into().unwrap(),
            esp_mountpoint.path(),
            gen_lnks,
        )?;
        assert!(output1.status.success());

        assert!(unrelated_loader_config.exists());
        assert!(unrelated_uki.exists());
        assert!(unrelated_os.exists());
        assert!(unrelated_firmware.exists());

        Ok(())
    };

    // Without profile
    let generation_links: Vec<PathBuf> = [1, 2, 3]
        .into_iter()
        .map(|v| {
            common::setup_generation_link(tmpdir.path(), profiles.path(), v, None)
                .expect("Failed to setup generation link")
        })
        .collect();
    let _ = kp_unrel_files(generation_links, usize::from(2u8));

    // With profile
    let generation_links_prof: Vec<PathBuf> = [
        ("My Prof 1", 1),
        ("My Prof 1", 2),
        ("My-Prof-2", 1),
        ("My-Prof-2", 2),
        ("My_Prof_3", 1),
        ("My_Prof_3", 2),
    ]
    .into_iter()
    .map(|(p, v)| {
        common::setup_generation_link(tmpdir.path(), profiles.path(), v, Some(p.to_string()))
            .expect("Failed to setup generation link")
    })
    .collect();
    let _ = kp_unrel_files(generation_links_prof, usize::from(4u8));

    Ok(())
}
