use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tempfile::{TempDir, tempdir};

use crate::common::{
    self, count_files, mtime, profile_image_path, setup_profile_generation_link_from_toplevel,
};

/// Count the stubs on the ESP whose file name starts with the prefix.
fn count_stubs_with_prefix(esp: &TempDir, prefix: &str) -> usize {
    fs::read_dir(esp.path().join("EFI/Linux"))
        .expect("Failed to read EFI/Linux")
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|e| e.file_name().to_string_lossy().starts_with(prefix))
        })
        .count()
}

/// Create the generation links of a profile, each with its own toplevel.
fn setup_profile_generations(
    tmpdir: &Path,
    profiles: &Path,
    profile: Option<&str>,
    versions: impl IntoIterator<Item = u64>,
) -> Vec<(PathBuf, PathBuf)> {
    versions
        .into_iter()
        .map(|version| {
            let toplevel = common::setup_toplevel(tmpdir).expect("Failed to setup toplevel");
            let link =
                setup_profile_generation_link_from_toplevel(&toplevel, profiles, profile, version)
                    .expect("Failed to setup generation link");
            (link, toplevel)
        })
        .collect()
}

#[test]
fn install_profile_only() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let custom = setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1, 2]);

    let output = common::lanzaboote_install(0, esp.path(), custom.iter().map(|(link, _)| link))?;
    assert!(output.status.success());

    for (version, (_, toplevel)) in [1, 2].into_iter().zip(&custom) {
        assert!(profile_image_path(&esp, Some("custom"), version, toplevel)?.exists());
    }
    // Each generation also has a specialisation.
    assert_eq!(count_stubs_with_prefix(&esp, "nixos-profile-custom-"), 4);
    assert_eq!(count_stubs_with_prefix(&esp, "nixos-generation-"), 0);

    Ok(())
}

/// Install generations of several profiles that share version numbers and even the toplevel.
/// Each of them must get its own stub, and reinstalling must recognize all of them.
#[test]
fn install_mixed_profiles_with_equal_versions() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;
    let other_toplevel = common::setup_toplevel(tmpdir.path())?;

    let generation_links = vec![
        setup_profile_generation_link_from_toplevel(&toplevel, profiles.path(), None, 1)?,
        setup_profile_generation_link_from_toplevel(&toplevel, profiles.path(), Some("custom"), 1)?,
        setup_profile_generation_link_from_toplevel(
            &other_toplevel,
            profiles.path(),
            Some("my-host-profile"),
            1,
        )?,
    ];
    let images = [
        profile_image_path(&esp, None, 1, &toplevel)?,
        profile_image_path(&esp, Some("custom"), 1, &toplevel)?,
        profile_image_path(&esp, Some("my-host-profile"), 1, &other_toplevel)?,
    ];

    let stub_count = || count_files(&esp.path().join("EFI/Linux")).unwrap();
    let kernel_and_initrd_count = || count_files(&esp.path().join("EFI/nixos")).unwrap();

    let output0 = common::lanzaboote_install(0, esp.path(), generation_links.clone())?;
    assert!(output0.status.success());
    assert_eq!(stub_count(), 6, "Wrong number of stubs after installation");
    assert_eq!(
        kernel_and_initrd_count(),
        2,
        "Wrong number of kernels & initrds after installation"
    );
    for image in &images {
        assert!(image.exists(), "{image:?} is missing");
    }

    let mtimes = images.iter().map(|i| mtime(i)).collect::<Vec<_>>();
    let output1 = common::lanzaboote_install(0, esp.path(), generation_links)?;
    assert!(output1.status.success());
    assert_eq!(
        stub_count(),
        6,
        "Wrong number of stubs after reinstallation"
    );
    assert_eq!(
        mtimes,
        images.iter().map(|i| mtime(i)).collect::<Vec<_>>(),
        "Stubs were rewritten on reinstallation"
    );

    Ok(())
}

#[test]
fn garbage_collect_deleted_profile_generation() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1]);
    let custom = setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1, 2]);

    let all_links = system.iter().chain(&custom).map(|(link, _)| link);
    let output0 = common::lanzaboote_install(0, esp.path(), all_links)?;
    assert!(output0.status.success());
    let custom_1 = profile_image_path(&esp, Some("custom"), 1, &custom[0].1)?;
    assert!(custom_1.exists());

    // Delete the first generation of the profile.
    fs::remove_dir_all(&custom[0].0)?;
    let remaining_links = system.iter().chain(&custom[1..]).map(|(link, _)| link);
    let output1 = common::lanzaboote_install(0, esp.path(), remaining_links)?;
    assert!(output1.status.success());

    assert!(!custom_1.exists());
    assert!(profile_image_path(&esp, Some("custom"), 2, &custom[1].1)?.exists());
    assert!(profile_image_path(&esp, None, 1, &system[0].1)?.exists());
    assert_eq!(count_stubs_with_prefix(&esp, "nixos-profile-custom-"), 2);

    Ok(())
}

/// Like systemd-boot-builder.py, the configuration limit applies to each profile separately.
#[test]
fn configuration_limit_applies_per_profile() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1, 2, 3]);
    let custom =
        setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1, 2, 3]);

    let all_links = system.iter().chain(&custom).map(|(link, _)| link);
    let output = common::lanzaboote_install(2, esp.path(), all_links)?;
    assert!(output.status.success());

    for (profile, generations) in [(None, &system), (Some("custom"), &custom)] {
        for (version, (_, toplevel)) in [1, 2, 3].into_iter().zip(generations) {
            let image = profile_image_path(&esp, profile, version, toplevel)?;
            assert_eq!(image.exists(), version > 1, "{image:?}");
        }
    }

    Ok(())
}

/// The protected system only replaces the oldest generation of its own profile.
#[test]
fn keep_protected_profile_generation() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1, 2, 3]);
    let custom =
        setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1, 2, 3]);

    let all_links = system.iter().chain(&custom).map(|(link, _)| link);
    let output = common::Lanzaboote::new()
        .config_limit(2)
        .protected_system(custom[0].0.clone())
        .install(esp.path(), all_links)?;
    assert!(output.status.success());

    for (profile, generations, kept) in [
        (None, &system, [false, true, true]),
        (Some("custom"), &custom, [true, false, true]),
    ] {
        for ((version, (_, toplevel)), kept) in [1, 2, 3].into_iter().zip(generations).zip(kept) {
            let image = profile_image_path(&esp, profile, version, toplevel)?;
            assert_eq!(image.exists(), kept, "{image:?}");
        }
    }

    Ok(())
}

/// A malformed generation of a profile disables garbage collection like one of the default
/// profile does.
#[test]
fn malformed_profile_generation_disables_gc() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1, 2]);

    let output0 = common::lanzaboote_install(0, esp.path(), system.iter().map(|(link, _)| link))?;
    assert!(output0.status.success());

    // A profile generation without a bootspec and without anything to synthesize one from.
    let malformed = profiles.path().join("system-profiles/custom-1-link");
    fs::create_dir_all(&malformed)?;

    let output1 = common::lanzaboote_install(0, esp.path(), [&system[1].0, &malformed])?;
    assert!(output1.status.success());

    // Without the malformed generation, generation 1 would have been garbage collected.
    assert!(profile_image_path(&esp, None, 1, &system[0].1)?.exists());
    let stderr = String::from_utf8(output1.stderr)?;
    assert!(
        stderr.contains(
            "nix-env -p /nix/var/nix/profiles/system-profiles/custom --delete-generations 1"
        ),
        "{stderr}"
    );

    Ok(())
}

/// Generation links of all profiles are discovered in the profiles directory. Everything else in
/// there is ignored.
#[test]
fn discover_generations_in_profiles_directory() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1, 2]);
    let custom = setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1]);

    // The profile symlinks, the profiles of nix-env and other files must be ignored. If any of
    // them were treated as a generation, it would be malformed and disable garbage collection.
    std::os::unix::fs::symlink("system-2-link", profiles.path().join("system"))?;
    std::os::unix::fs::symlink(
        "custom-1-link",
        profiles.path().join("system-profiles/custom"),
    )?;
    fs::create_dir(profiles.path().join("default-1-link"))?;
    fs::create_dir(profiles.path().join("per-user"))?;
    fs::write(profiles.path().join("system-profiles/notes.txt"), b"")?;

    let output = common::Lanzaboote::new()
        .profiles_directory(profiles.path().to_path_buf())
        .install(esp.path(), Vec::<PathBuf>::new())?;
    assert!(output.status.success());
    assert!(
        !String::from_utf8(output.stderr)?.contains("Garbage collection is disabled"),
        "Something other than a generation link was treated as a generation"
    );

    for (version, (_, toplevel)) in [1, 2].into_iter().zip(&system) {
        assert!(profile_image_path(&esp, None, version, toplevel)?.exists());
    }
    assert!(profile_image_path(&esp, Some("custom"), 1, &custom[0].1)?.exists());
    // Each generation also has a specialisation.
    assert_eq!(count_files(&esp.path().join("EFI/Linux"))?, 6);

    Ok(())
}

/// A profiles directory without system-profiles, like on most hosts, works as before.
#[test]
fn discover_generations_without_system_profiles() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1]);

    let output = common::Lanzaboote::new()
        .profiles_directory(profiles.path().to_path_buf())
        .install(esp.path(), Vec::<PathBuf>::new())?;
    assert!(output.status.success());
    assert!(profile_image_path(&esp, None, 1, &system[0].1)?.exists());
    assert_eq!(count_stubs_with_prefix(&esp, "nixos-generation-"), 2);

    Ok(())
}

/// Profile names that cannot be used in file names on the ESP are rejected before anything is
/// installed.
#[test]
fn reject_unsafe_profile_name() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1]);
    let unsafe_profile =
        setup_profile_generations(tmpdir.path(), profiles.path(), Some("my profile"), [1]);

    let links = system.iter().chain(&unsafe_profile).map(|(link, _)| link);
    let output = common::lanzaboote_install(0, esp.path(), links)?;
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("contains characters"), "{stderr}");
    assert!(!esp.path().join("EFI/Linux").exists());

    Ok(())
}

/// A discovered profile with a name that cannot be used on the ESP is skipped with a warning
/// instead of failing the installation of the other generations.
#[test]
fn skip_unsafe_profile_name_on_discovery() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1]);
    setup_profile_generations(tmpdir.path(), profiles.path(), Some("my profile"), [1]);

    let output = common::Lanzaboote::new()
        .profiles_directory(profiles.path().to_path_buf())
        .install(esp.path(), Vec::<PathBuf>::new())?;
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("Ignoring generation link"), "{stderr}");
    assert!(
        !stderr.contains("Garbage collection is disabled"),
        "{stderr}"
    );

    assert!(profile_image_path(&esp, None, 1, &system[0].1)?.exists());
    assert_eq!(count_stubs_with_prefix(&esp, "nixos-profile-"), 0);

    Ok(())
}

/// Set distinct modification times on generation links, oldest first.
fn set_build_order<'a>(links: impl IntoIterator<Item = &'a PathBuf>) {
    for (i, link) in links.into_iter().enumerate() {
        filetime::set_file_mtime(
            link,
            filetime::FileTime::from_unix_time(1_000_000 * (i as i64 + 1), 0),
        )
        .expect("Failed to set mtime");
    }
}

/// With Measured Boot, the configuration limit applies to the generations of all profiles together,
/// keeping the most recently built ones, and the protected generation evicts the oldest one overall.
#[test]
fn configuration_limit_is_global_with_measured_boot() -> Result<()> {
    let esp = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let pcrlock = tempdir()?;
    let system = setup_profile_generations(tmpdir.path(), profiles.path(), None, [1, 2, 3]);
    let custom = setup_profile_generations(tmpdir.path(), profiles.path(), Some("custom"), [1, 2]);
    // Build order: system 1, system 2, custom 1, system 3, custom 2.
    set_build_order([
        &system[0].0,
        &system[1].0,
        &custom[0].0,
        &system[2].0,
        &custom[1].0,
    ]);

    let all_links = || system.iter().chain(&custom).map(|(link, _)| link.clone());
    let output = common::Lanzaboote::new()
        .config_limit(3)
        .pcrlock_directory(pcrlock.path().to_path_buf())
        .install(esp.path(), all_links())?;
    assert!(output.status.success());

    for (profile, generations, kept) in [
        (None, &system, [false, false, true]),
        (Some("custom"), &custom, [true, true, false]),
    ] {
        for ((version, (_, toplevel)), kept) in (1..).zip(generations).zip(kept) {
            let image = profile_image_path(&esp, profile, version, toplevel)?;
            assert_eq!(image.exists(), kept, "{image:?}");
        }
    }

    // Protecting the oldest generation evicts the oldest of the kept ones, regardless of profile.
    let output = common::Lanzaboote::new()
        .config_limit(3)
        .pcrlock_directory(pcrlock.path().to_path_buf())
        .protected_system(system[0].0.clone())
        .install(esp.path(), all_links())?;
    assert!(output.status.success());

    for (profile, generations, kept) in [
        (None, &system, [true, false, true]),
        (Some("custom"), &custom, [false, true, false]),
    ] {
        for ((version, (_, toplevel)), kept) in (1..).zip(generations).zip(kept) {
            let image = profile_image_path(&esp, profile, version, toplevel)?;
            assert_eq!(image.exists(), kept, "{image:?}");
        }
    }

    Ok(())
}
