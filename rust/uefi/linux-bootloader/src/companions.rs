use crate::cpio::{pack_cpio, Cpio};
use alloc::{string::ToString, vec::Vec};
use uefi::{
    cstr16,
    fs::{Path, PathBuf},
    proto::device_path::{
        text::{AllowShortcuts, DisplayOnly},
        DevicePath,
    },
    table, CString16,
};

/// Locate files with ASCII filenames and matching the suffix passed as a parameter.
/// Returns a list of their paths.
pub fn find_files(
    fs: &mut uefi::fs::FileSystem,
    search_path: &Path,
    suffix: &str,
) -> uefi::Result<Vec<PathBuf>> {
    let mut results = Vec::new();

    for maybe_entry in fs.read_dir(search_path).unwrap() {
        let entry = maybe_entry?;
        if entry.is_regular_file() {
            let fname = entry.file_name();
            if fname.is_ascii() && fname.to_string().ends_with(suffix) {
                let mut full_path = CString16::from(search_path.to_cstr16());
                full_path.push_str(cstr16!("\\"));
                full_path.push_str(fname);
                results.push(full_path.into());
            }
        }
    }

    Ok(results)
}

/// Returns the "default" drop-in directory if it exists.
/// This will be in general $loaded_image_path.extra/
pub fn get_default_dropin_directory(
    loaded_image_file_path: &DevicePath,
    fs: &mut uefi::fs::FileSystem,
) -> uefi::Result<Option<PathBuf>> {
    // We could use LoadedImageDevicePath to get the full device path
    // and perform replacement of the last node before END_ENTIRE
    // by another node containing the filename + .extra
    // But this is as much tedious as performing a conversion to string
    // then opening the root directory and finding the new directory.
    let mut target_directory = loaded_image_file_path
        .to_string(
            table::system_table_boot().unwrap().boot_services(),
            DisplayOnly(false),
            AllowShortcuts(false),
        )
        .map_err(|_dpp_error| {
            log::warn!("Failed to obtain string representation of the loaded image file path");
            uefi::Error::new(uefi::Status::NOT_FOUND, ())
        })?;
    target_directory.push_str(cstr16!(".extra"));

    Ok(fs
        .metadata(target_directory.as_ref())
        .ok()
        .and_then(|metadata| {
            metadata
                .is_directory()
                .then(|| PathBuf::from(target_directory))
        }))
}

pub enum CompanionInitrdType {
    Credentials,
    GlobalCredentials,
    SystemExtension,
    PcrSignature,
    PcrPublicKey,
}

/// Potential companion initrd assembled on the fly
/// during discovery workflows, e.g. finding files in drop-in directories.
pub struct CompanionInitrd {
    pub r#type: CompanionInitrdType,
    pub cpio: Cpio,
}

/// Collect all credentials and return them as CPIO archive.
///
/// There are two variants of credentials:
///   - global: `$ESP/loader.credentials/*.cred`
///   - image-specific: `$path_to_image.extra/*.cred`
///
/// The credentials are not measured.
pub fn discover_credentials(
    fs: &mut uefi::fs::FileSystem,
    default_dropin_dir: Option<&Path>,
) -> uefi::Result<Vec<CompanionInitrd>> {
    let mut companions = Vec::new();

    let default_global_dropin_dir = cstr16!("\\loader\\credentials");
    if fs.try_exists(default_global_dropin_dir).unwrap() {
        let metadata = fs.metadata(default_global_dropin_dir).map_err(|_err| {
            log::warn!("Failed to obtain metadata on `\\loader\\credentials` path (which is supposed to exist)");
            uefi::Error::new(uefi::Status::VOLUME_CORRUPTED, ())
        })?;
        if metadata.is_directory() {
            let global_credentials: Vec<PathBuf> =
                find_files(fs, default_global_dropin_dir.as_ref(), ".cred")?;

            if !global_credentials.is_empty() {
                companions.push(CompanionInitrd {
                    r#type: CompanionInitrdType::GlobalCredentials,
                    cpio: pack_cpio(
                        fs,
                        global_credentials,
                        ".extra/global_credentials",
                        0o500,
                        0o400,
                    )
                    .map_err(|_err| uefi::Status::LOAD_ERROR)?,
                });
            }
        }
    }

    if let Some(default_dropin_dir) = default_dropin_dir {
        let local_credentials: Vec<PathBuf> = find_files(fs, default_dropin_dir, ".cred")?;

        if !local_credentials.is_empty() {
            companions.push(CompanionInitrd {
                r#type: CompanionInitrdType::Credentials,
                cpio: pack_cpio(fs, local_credentials, ".extra/credentials", 0o500, 0o400)
                    .map_err(|_err| uefi::Status::LOAD_ERROR)?,
            });
        }
    }

    Ok(companions)
}
/// Discover any system image extension, i.e. files ending by .raw
/// They must be present inside $path_to_image.extra/*.raw, specific to this image.
///
/// Those will be unmeasured, you are responsible for measuring them or not.
/// But CPIOs are guaranteed to be stable and independent of file discovery order.
pub fn discover_system_extensions(
    fs: &mut uefi::fs::FileSystem,
    default_dropin_dir: &Path,
) -> uefi::Result<Vec<CompanionInitrd>> {
    let mut companions = Vec::new();
    let sysexts = find_files(fs, default_dropin_dir, ".raw")?;

    if !sysexts.is_empty() {
        companions.push(CompanionInitrd {
            r#type: CompanionInitrdType::SystemExtension,
            cpio: pack_cpio(fs, sysexts, ".extra/sysext", 0o555, 0o444)
                .map_err(|_err| uefi::Status::LOAD_ERROR)?,
        });
    }

    Ok(companions)
}
