use std::path::PathBuf;

use anyhow::Result;
use base32ct::{Base32Unpadded, Encoding};
use tempfile::{tempdir, TempDir};

mod common;

use common::{
    count_files, hash_file, remove_signature, setup_generation_link_from_toplevel, verify_signature,
};

/// Install two generations that point at the same toplevel.
/// This should install two lanzaboote images and one kernel and one initrd.
#[test]
fn do_not_install_duplicates() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let generation_link1 = setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1)?;
    let generation_link2 = setup_generation_link_from_toplevel(&toplevel, profiles.path(), 2)?;
    let generation_links = vec![generation_link1, generation_link2];

    let stub_count = || count_files(&esp.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp.path().join("EFI/nixos")).unwrap();

    let output1 = common::lanzaboote_install(0, esp.path(), generation_links)?;
    assert!(output1.status.success());
    assert_eq!(stub_count(), 2, "Wrong number of stubs after installation");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after installation"
    );
    Ok(())
}

#[test]
fn overwrite_unsigned_images() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;

    let image1 = image_path(&esp, 1);
    let image2 = image_path(&esp, 2);

    let generation_link1 = common::setup_generation_link(tmpdir.path(), profiles.path(), 1)?;
    let generation_link2 = common::setup_generation_link(tmpdir.path(), profiles.path(), 2)?;
    let generation_links = vec![generation_link1, generation_link2];

    let output1 = common::lanzaboote_install(0, esp.path(), generation_links.clone())?;
    assert!(output1.status.success());

    remove_signature(&image1)?;
    assert!(!verify_signature(&image1)?);
    assert!(verify_signature(&image2)?);

    let output2 = common::lanzaboote_install(0, esp.path(), generation_links)?;
    assert!(output2.status.success());

    assert!(verify_signature(&image1)?);
    assert!(verify_signature(&image2)?);

    Ok(())
}

#[test]
fn content_addressing_works() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let generation_link = setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1)?;
    let generation_links = vec![generation_link];

    let kernel_hash_source =
        hash_file(&toplevel.join("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-6.1.1/kernel"));

    let output0 = common::lanzaboote_install(1, esp.path(), generation_links)?;
    assert!(output0.status.success());

    let kernel_path = esp.path().join(format!(
        "EFI/nixos/kernel-6.1.1-{}.efi",
        Base32Unpadded::encode_string(&kernel_hash_source)
    ));

    // Implicitly assert that the content-addressed file actually exists.
    let kernel_hash = hash_file(&kernel_path);
    // Assert the written kernel is the source kernel.
    assert_eq!(kernel_hash_source, kernel_hash);

    Ok(())
}

fn image_path(esp: &TempDir, version: u64) -> PathBuf {
    esp.path()
        .join(format!("EFI/Linux/nixos-generation-{version}.efi"))
}
