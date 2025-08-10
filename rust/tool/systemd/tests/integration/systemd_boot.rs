use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use lanzaboote_tool::architecture::Architecture;
use lzbt_systemd::architecture::SystemdArchitectureExt;
use tempfile::tempdir;

use crate::common::{self, SYSTEM, hash_file, mtime, remove_signature, verify_signature};

#[test]
fn keep_systemd_boot_binaries() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let systemd_boot_path = systemd_boot_path(&esp);
    let systemd_boot_fallback_path = systemd_boot_fallback_path(&esp);

    let kp_sys_boot_bins = |prof: Option<String>| -> Result<()> {
        let generation_link =
            common::setup_generation_link(tmpdir.path(), profiles.path(), 1, prof)
                .expect("Failed to setup generation link");

        let output0 = common::lanzaboote_install(0, esp.path(), vec![&generation_link])?;
        assert!(output0.status.success());

        // Use the modification time instead of a hash because the hash would be the same even if the
        // file was overwritten.
        let systemd_boot_mtime0 = mtime(&systemd_boot_path);
        let systemd_boot_fallback_mtime0 = mtime(&systemd_boot_fallback_path);

        let output1 = common::lanzaboote_install(0, esp.path(), vec![generation_link])?;
        assert!(output1.status.success());

        let systemd_boot_mtime1 = mtime(&systemd_boot_path);
        let systemd_boot_fallback_mtime1 = mtime(&systemd_boot_fallback_path);

        assert_eq!(
            systemd_boot_mtime0, systemd_boot_mtime1,
            "systemd-boot binary was modified on second install."
        );
        assert_eq!(
            systemd_boot_fallback_mtime0, systemd_boot_fallback_mtime1,
            "systemd-boot fallback binary was moidified on second install."
        );

        Ok(())
    };

    // Without profile
    let _ = kp_sys_boot_bins(None);

    // With profile
    let _ = kp_sys_boot_bins(Some("MyProfile".to_string()));

    Ok(())
}

#[test]
fn overwrite_malformed_systemd_boot_binaries() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let systemd_boot_path = systemd_boot_path(&esp);
    let systemd_boot_fallback_path = systemd_boot_fallback_path(&esp);

    let ow_mal_sys_boot_bins = |prof: Option<String>| -> Result<()> {
        let generation_link =
            common::setup_generation_link(tmpdir.path(), profiles.path(), 1, prof)
                .expect("Failed to setup generation link");

        let output0 = common::lanzaboote_install(0, esp.path(), vec![&generation_link])?;
        assert!(output0.status.success());

        // Make systemd-boot binaries malformed by truncating them.
        fs::File::create(&systemd_boot_path)?;
        fs::File::create(&systemd_boot_fallback_path)?;

        let malformed_systemd_boot_hash = hash_file(&systemd_boot_path);
        let malformed_systemd_boot_fallback_hash = hash_file(&systemd_boot_fallback_path);

        let output1 = common::lanzaboote_install(0, esp.path(), vec![generation_link])?;
        assert!(output1.status.success());

        let systemd_boot_hash = hash_file(&systemd_boot_path);
        let systemd_boot_fallback_hash = hash_file(&systemd_boot_fallback_path);

        assert_ne!(
            malformed_systemd_boot_hash, systemd_boot_hash,
            "Malformed systemd-boot binaries were not replaced."
        );
        assert_ne!(
            malformed_systemd_boot_fallback_hash, systemd_boot_fallback_hash,
            "Maformed systemd-boot fallback binaries were not replaced."
        );

        Ok(())
    };

    // Without profile
    let _ = ow_mal_sys_boot_bins(None);

    // With profile
    let _ = ow_mal_sys_boot_bins(Some("MyProfile".to_string()));

    Ok(())
}

#[test]
fn overwrite_unsigned_systemd_boot_binaries() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let systemd_boot_path = systemd_boot_path(&esp);
    let systemd_boot_fallback_path = systemd_boot_fallback_path(&esp);

    let ow_unsign_sys_boot_bins = |prof: Option<String>| -> Result<()> {
        let generation_link =
            common::setup_generation_link(tmpdir.path(), profiles.path(), 1, prof)
                .expect("Failed to setup generation link");

        let output0 = common::lanzaboote_install(0, esp.path(), vec![&generation_link])?;
        assert!(output0.status.success());

        remove_signature(&systemd_boot_path)?;
        remove_signature(&systemd_boot_fallback_path)?;
        assert!(!verify_signature(&systemd_boot_path)?);
        assert!(!verify_signature(&systemd_boot_fallback_path)?);

        let output1 = common::lanzaboote_install(0, esp.path(), vec![generation_link])?;
        assert!(output1.status.success());

        assert!(verify_signature(&systemd_boot_path)?);
        assert!(verify_signature(&systemd_boot_fallback_path)?);

        Ok(())
    };

    // Without profile
    let _ = ow_unsign_sys_boot_bins(None);

    // With profile
    let _ = ow_unsign_sys_boot_bins(Some("MyProfile".to_string()));

    Ok(())
}

fn systemd_boot_path(esp: &tempfile::TempDir) -> PathBuf {
    let arch = Architecture::from_nixos_system(SYSTEM).unwrap();
    esp.path()
        .join("EFI/systemd/")
        .join(arch.systemd_filename())
}

fn systemd_boot_fallback_path(esp: &tempfile::TempDir) -> PathBuf {
    let arch = Architecture::from_nixos_system(SYSTEM).unwrap();
    esp.path()
        .join("EFI/BOOT/")
        .join(arch.efi_fallback_filename())
}
