// Utility code in this module can become marked as dead code if it is not used in every single
// module in `tests/`. Thus we need to allow dead code here. See
// https://stackoverflow.com/a/67902444
#![allow(dead_code)]

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::prelude::{MetadataExt, OsStrExt};
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Context, Result};
use assert_cmd::Command;
use base32ct::{Base32Unpadded, Encoding};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use lanzaboote_tool::architecture::Architecture;

/// Returns the host platform system
/// in the system double format for
/// our usual targets.
#[cfg(target_arch = "aarch64")]
pub static SYSTEM: &str = "aarch64-linux";

// We do not actually care much about 32 bit. However we can use this to easily test that lzbt
// works with another architecture.
#[cfg(target_arch = "x86")]
pub static SYSTEM: &str = "i686-linux";

#[cfg(target_arch = "x86_64")]
pub static SYSTEM: &str = "x86_64-linux";

/// Create a mock generation link.
///
/// Works like `setup_generation_link_from_toplevel` but already sets up toplevel.
pub fn setup_generation_link(
    tmpdir: &Path,
    profiles_directory: &Path,
    version: u64,
) -> Result<PathBuf> {
    let toplevel = setup_toplevel(tmpdir).context("Failed to setup toplevel")?;
    setup_generation_link_from_toplevel(&toplevel, profiles_directory, version)
}

/// Create a mock generation link.
///
/// Creates the generation link using the specified version inside a mock profiles directory
/// (mimicking /nix/var/nix/profiles). Returns the path to the generation link.
pub fn setup_generation_link_from_toplevel(
    toplevel: &Path,
    profiles_directory: &Path,
    version: u64,
) -> Result<PathBuf> {
    let bootspec = json!({
        "org.nixos.bootspec.v1": {
          "init": format!("init-v{}", version),
          // Normally, these are in the Nix store.
          "initrd": toplevel.join("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-6.1.1/initrd"),
          "kernel": toplevel.join("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-6.1.1/kernel"),
          "kernelParams": [
            "amd_iommu=on",
            "amd_iommu=pt",
            "iommu=pt",
            "kvm.ignore_msrs=1",
            "kvm.report_ignored_msrs=0",
            "udev.log_priority=3",
            "systemd.unified_cgroup_hierarchy=1",
            "loglevel=4"
          ],
          "label": "LanzaOS",
          "toplevel": toplevel,
          "system": SYSTEM,
        },
        "org.nixos-community.lanzaboote": { "osRelease": toplevel.join("os-release") }
    });

    let generation_link_path = profiles_directory.join(format!("system-{}-link", version));
    fs::create_dir(&generation_link_path)?;

    let bootspec_path = generation_link_path.join("boot.json");
    let mut file = fs::File::create(bootspec_path)?;
    file.write_all(&serde_json::to_vec(&bootspec)?)?;

    // Explicitly set modification time so that snapshot test of os-release reliably works.
    // This has to happen after any modifications to the directory.
    filetime::set_file_mtime(&generation_link_path, filetime::FileTime::zero())?;
    Ok(generation_link_path)
}

/// Setup a mock toplevel inside a temporary directory.
///
/// Accepts the temporary directory as a parameter so that the invoking function retains control of
/// it (and when it goes out of scope).
pub fn setup_toplevel(tmpdir: &Path) -> Result<PathBuf> {
    let architecture = Architecture::from_nixos_system(SYSTEM)?;

    // Generate a random toplevel name so that multiple toplevel paths can live alongside each
    // other in the same directory.
    let toplevel = tmpdir.join(format!("toplevel-{}", random_string(8)));
    let fake_store_path = toplevel.join("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-6.1.1");
    fs::create_dir_all(&fake_store_path)?;

    let test_systemd = systemd_location_from_env()?;
    let systemd_stub_filename = systemd_stub_filename(&architecture);
    let test_systemd_stub = format!(
        "{test_systemd}/lib/systemd/boot/efi/{systemd_stub_filename}",
        systemd_stub_filename = systemd_stub_filename.display()
    );

    let initrd_path = fake_store_path.join("initrd");
    let kernel_path = fake_store_path.join("kernel");
    let nixos_version_path = toplevel.join("nixos-version");
    let kernel_modules_path = toplevel.join("kernel-modules/lib/modules/6.1.1");

    // To simplify the test setup, we use the systemd stub for all PE binaries used by lanzatool.
    // Lanzatool doesn't care whether its actually a kernel or initrd but only whether it can
    // manipulate the PE binary with objcopy and/or sign it with sbsigntool. For testing lanzatool
    // in isolation this should suffice.
    fs::copy(&test_systemd_stub, initrd_path)?;
    fs::copy(&test_systemd_stub, kernel_path)?;
    fs::write(nixos_version_path, b"23.05")?;
    fs::create_dir_all(kernel_modules_path)?;

    Ok(toplevel)
}

fn random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Call the `lanzaboote install` command.
pub fn lanzaboote_install(
    config_limit: u64,
    esp_mountpoint: &Path,
    generation_links: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<Output> {
    // To simplify the test setup, we use the systemd stub here instead of the lanzaboote stub. See
    // the comment in setup_toplevel for details.
    let architecture = Architecture::from_nixos_system(SYSTEM)?;
    let test_systemd = systemd_location_from_env()?;
    let systemd_stub_filename = systemd_stub_filename(&architecture);
    let test_systemd_stub = format!(
        "{test_systemd}/lib/systemd/boot/efi/{systemd_stub_filename}",
        systemd_stub_filename = systemd_stub_filename.display()
    );

    let test_loader_config_path = tempfile::NamedTempFile::new()?;
    let test_loader_config = r"timeout 0\nconsole-mode 1\n";
    fs::write(test_loader_config_path.path(), test_loader_config)?;

    let mut cmd = Command::cargo_bin("lzbt-systemd")?;
    let output = cmd
        .env("LANZABOOTE_STUB", test_systemd_stub)
        .arg("-vv")
        .arg("install")
        .arg("--system")
        .arg(SYSTEM)
        .arg("--systemd")
        .arg(test_systemd)
        .arg("--systemd-boot-loader-config")
        .arg(test_loader_config_path.path())
        .arg("--public-key")
        .arg("tests/fixtures/uefi-keys/db.pem")
        .arg("--private-key")
        .arg("tests/fixtures/uefi-keys/db.key")
        .arg("--configuration-limit")
        .arg(config_limit.to_string())
        .arg(esp_mountpoint)
        .args(generation_links)
        .output()?;

    // Print debugging output.
    // This is a weird hack to make cargo test capture the output.
    // See https://github.com/rust-lang/rust/issues/12309
    print!("{}", String::from_utf8(output.stdout.clone())?);
    print!("{}", String::from_utf8(output.stderr.clone())?);

    // Also walk the entire ESP mountpoint and print each path for debugging
    for entry in walkdir::WalkDir::new(esp_mountpoint) {
        println!("{}", entry?.path().display());
    }

    Ok(output)
}

/// Read location of systemd installation from an environment variable.
fn systemd_location_from_env() -> Result<String> {
    let error_msg = "TEST_SYSTEMD environment variable is not set. TEST_SYSTEMD has to point to a systemd installation.
On a system with Nix installed, you can set it with: export TEST_SYSTEMD=$(nix-build '<nixpkgs>' -A systemd)";
    std::env::var("TEST_SYSTEMD").context(error_msg)
}

/// Look up the modification time (mtime) of a file.
pub fn mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .expect("Failed to read modification time.")
        .mtime()
}

pub fn hash_file(path: &Path) -> sha2::digest::Output<Sha256> {
    Sha256::digest(fs::read(path).expect("Failed to read file to hash."))
}

/// Remove signature from a signed PE file.
pub fn remove_signature(path: &Path) -> Result<()> {
    let output = Command::new("sbattach")
        .arg("--remove")
        .arg(path.as_os_str())
        .output()
        .context("Failed to run sbattach. Most likely, the binary is not on PATH.")?;
    print!("{}", String::from_utf8(output.stdout)?);
    print!("{}", String::from_utf8(output.stderr)?);
    Ok(())
}

/// Verify signature of PE file.
pub fn verify_signature(path: &Path) -> Result<bool> {
    let output = Command::new("sbverify")
        .arg(path.as_os_str())
        .arg("--cert")
        .arg("tests/fixtures/uefi-keys/db.pem")
        .output()
        .context("Failed to run sbverify. Most likely, the binary is not on PATH.")?;
    print!("{}", String::from_utf8(output.stdout)?);
    print!("{}", String::from_utf8(output.stderr)?);
    Ok(output.status.success())
}

pub fn count_files(path: &Path) -> Result<usize> {
    Ok(fs::read_dir(path)?.count())
}

pub fn image_path(esp: &TempDir, version: u64, toplevel: &Path) -> Result<PathBuf> {
    let stub_inputs = [
        // Generation numbers can be reused if the latest generation was deleted.
        // To detect this, the stub path depends on the actual toplevel used.
        ("toplevel", toplevel.as_os_str().as_bytes()),
        // If the key is rotated, the signed stubs must be re-generated.
        // So we make their path depend on the public key used for signature.
        (
            "public_key",
            &std::fs::read("tests/fixtures/uefi-keys/db.pem")?,
        ),
    ];
    let stub_input_hash = Base32Unpadded::encode_string(&Sha256::digest(
        serde_json::to_string(&stub_inputs).unwrap(),
    ));
    Ok(esp.path().join(format!(
        "EFI/Linux/nixos-generation-{version}-{stub_input_hash}.efi"
    )))
}

fn systemd_stub_filename(architecture: &Architecture) -> PathBuf {
    format!("linux{}.efi.stub", architecture.efi_representation()).into()
}
