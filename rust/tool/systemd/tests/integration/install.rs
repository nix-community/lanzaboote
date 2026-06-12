use anyhow::Result;
use base32ct::{Base32Unpadded, Encoding};
use tempfile::tempdir;

use crate::common::{
    self, count_files, hash_file, remove_signature, setup_generation_link_from_toplevel,
    verify_signature,
};

/// Install two generations that point at the same toplevel.
/// This should install two lanzaboote images and one kernel and one initrd.
#[test]
fn do_not_install_duplicates() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let stub_count = || count_files(&esp.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp.path().join("EFI/nixos")).unwrap();

    let no_inst_dups = |prof: Option<String>| -> Result<()> {
        let generation_link1 =
            setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1, prof.clone())?;
        let generation_link2 =
            setup_generation_link_from_toplevel(&toplevel, profiles.path(), 2, prof)?;
        let generation_links = vec![generation_link1, generation_link2];

        let output1 = common::lanzaboote_install(0, esp.path(), generation_links)?;
        assert!(output1.status.success());
        assert_eq!(stub_count(), 4, "Wrong number of stubs after installation");
        assert_eq!(
            kernel_and_initrd_count(),
            2,
            "Wrong number of kernels & initrds after installation"
        );

        Ok(())
    };

    // Without profile
    let _ = no_inst_dups(None);

    // With profile
    let _ = no_inst_dups(Some("MyProfile".to_string()));

    Ok(())
}

#[test]
fn do_not_overwrite_images() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let no_ow_imgs = |prof: Option<String>| -> Result<()> {
        let image1 = common::image_path(&esp, 1, prof.clone(), &toplevel)?;
        let image2 = common::image_path(&esp, 2, prof.clone(), &toplevel)?;

        let generation_link1 =
            setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1, prof.clone())?;
        let generation_link2 =
            setup_generation_link_from_toplevel(&toplevel, profiles.path(), 2, prof)?;
        let generation_links = vec![generation_link1, generation_link2];

        let output1 = common::lanzaboote_install(0, esp.path(), generation_links.clone())?;
        assert!(output1.status.success());

        assert!(verify_signature(&image1)?);
        remove_signature(&image1)?;
        assert!(!verify_signature(&image1)?);
        assert!(verify_signature(&image2)?);

        let output2 = common::lanzaboote_install(0, esp.path(), generation_links)?;
        assert!(output2.status.success());

        assert!(!verify_signature(&image1)?);
        assert!(verify_signature(&image2)?);

        Ok(())
    };

    // Without profile
    let _ = no_ow_imgs(None);

    // With profile
    let _ = no_ow_imgs(Some("MyProfile".to_string()));

    Ok(())
}

#[test]
fn detect_generation_number_reuse() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel1 = common::setup_toplevel(tmpdir.path())?;
    let toplevel2 = common::setup_toplevel(tmpdir.path())?;

    let det_gen_num_reuse = |prof: Option<String>| -> Result<()> {
        let image1 = common::image_path(&esp, 1, prof.clone(), &toplevel1)?;
        // this deliberately gets the same number!
        let image2 = common::image_path(&esp, 1, prof.clone(), &toplevel2)?;

        let generation_link1 =
            setup_generation_link_from_toplevel(&toplevel1, profiles.path(), 1, prof.clone())?;
        let output1 = common::lanzaboote_install(0, esp.path(), vec![generation_link1])?;
        assert!(output1.status.success());
        assert!(image1.exists());
        assert!(!image2.exists());

        std::fs::remove_dir_all(profiles.path().join(if let Some(ref p) = prof {
            format!("system-profiles/{}-1-link", p)
        } else {
            "system-1-link".to_string()
        }))?;
        let generation_link2 =
            setup_generation_link_from_toplevel(&toplevel2, profiles.path(), 1, prof)?;
        let output2 = common::lanzaboote_install(0, esp.path(), vec![generation_link2])?;
        assert!(output2.status.success());
        assert!(!image1.exists());
        assert!(image2.exists());

        Ok(())
    };

    // Without profile
    let _ = det_gen_num_reuse(None);

    // With profile
    let _ = det_gen_num_reuse(Some("MyProfile".to_string()));

    Ok(())
}

#[test]
fn content_addressing_works() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let generation_link = setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1, None)?;
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
