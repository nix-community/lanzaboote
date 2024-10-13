#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod common;

#[cfg(feature = "fat")]
mod fat;

#[cfg(feature = "thin")]
mod thin;

#[cfg(all(feature = "fat", feature = "thin"))]
compile_error!("A thin and fat stub cannot be produced at the same time, disable either `thin` or `fat` feature");

use alloc::vec::Vec;
use linux_bootloader::companions::{
    discover_credentials, discover_system_extensions, get_default_dropin_directory,
};
use linux_bootloader::efivars::{export_efi_variables, get_loader_features, EfiLoaderFeatures};
use linux_bootloader::measure::{measure_companion_initrds, measure_image};
use linux_bootloader::tpm::tpm_available;
use linux_bootloader::uefi_helpers::booted_image_file;
use log::{info, warn};
use uefi::prelude::*;

/// Lanzaboote stub name
pub static STUB_NAME: &str = concat!("lanzastub ", env!("CARGO_PKG_VERSION"));

/// Print the startup logo on boot.
fn print_logo() {
    info!(
        "
  _                      _                 _
 | |                    | |               | |
 | | __ _ _ __  ______ _| |__   ___   ___ | |_ ___
 | |/ _` | '_ \\|_  / _` | '_ \\ / _ \\ / _ \\| __/ _ \\
 | | (_| | | | |/ / (_| | |_) | (_) | (_) | ||  __/
 |_|\\__,_|_| |_/___\\__,_|_.__/ \\___/ \\___/ \\__\\___|

"
    );
}

#[entry]
fn main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init().unwrap();

    print_logo();

    let is_tpm_available = tpm_available(system_table.boot_services());
    let pe_in_memory = booted_image_file(system_table.boot_services())
        .expect("Failed to extract the in-memory information about our own image");

    if is_tpm_available {
        info!("TPM available, will proceed to measurements.");
        // Iterate over unified sections and measure them
        // For now, ignore failures during measurements.
        // TODO: in the future, devise a threat model where this can fail
        // and ensure this hard-fail correctly.
        let _ = measure_image(&system_table, &pe_in_memory);
    }

    if let Ok(features) = get_loader_features() {
        if !features.contains(EfiLoaderFeatures::RandomSeed) {
            // FIXME: process random seed then on the disk.
            info!("Random seed is available, but lanzaboote does not support it yet.");
        }
    }

    if export_efi_variables(STUB_NAME, &system_table).is_err() {
        warn!("Failed to export stub EFI variables, some features related to measured boot will not be available");
    }

    let status;
    // A list of dynamically assembled initrds, e.g. credential initrds or system extension
    // initrds.
    let mut dynamic_initrds: Vec<Vec<u8>> = Vec::new();

    {
        // This is a block for doing filesystem operations once and for all, related to companion
        // files, nothing can open the LoadedImage protocol here.
        // Everything must use `filesystem`.
        let mut companions = Vec::new();
        let image_fs = system_table
            .boot_services()
            .get_image_file_system(system_table.boot_services().image_handle());

        if let Ok(image_fs) = image_fs {
            let mut filesystem = uefi::fs::FileSystem::new(image_fs);
            let default_dropin_directory;

            if let Some(loaded_image_path) = pe_in_memory.file_path() {
                let discovered_default_dropin_dir = get_default_dropin_directory(
                    system_table.boot_services(),
                    loaded_image_path,
                    &mut filesystem,
                );

                if discovered_default_dropin_dir.is_err() {
                    warn!("Failed to discover the default drop-in directory for companion files");
                }

                default_dropin_directory = discovered_default_dropin_dir.unwrap_or(None);
            } else {
                default_dropin_directory = None;
            }

            // TODO: how to do the proper .as_ref()? Should I take AsRef in the call definition… ?
            if let Ok(mut system_credentials) = discover_credentials(
                &mut filesystem,
                default_dropin_directory.as_ref().map(|x| x.as_ref()),
            ) {
                companions.append(&mut system_credentials);
            } else {
                warn!("Failed to discover any system credential");
            }

            if let Some(default_dropin_dir) = default_dropin_directory {
                if let Ok(mut system_extensions) =
                    discover_system_extensions(&mut filesystem, &default_dropin_dir)
                {
                    companions.append(&mut system_extensions);
                } else {
                    warn!("Failed to discover any system extension");
                }
            }

            if is_tpm_available {
                // TODO: in the future, devise a threat model where this can fail, see above
                // measurements to understand the context.
                let _ = measure_companion_initrds(&system_table, &companions);
            }

            dynamic_initrds.append(
                &mut companions
                    .into_iter()
                    .map(|initrd| initrd.cpio.into_inner())
                    .collect(),
            );
        } else {
            warn!("Failed to open the simple filesystem for the booted image, this is expected for netbooted systems, skipping companion extension...");
        }
    }

    #[cfg(feature = "fat")]
    {
        status = fat::boot_linux(handle, system_table, dynamic_initrds)
    }

    #[cfg(feature = "thin")]
    {
        status = thin::boot_linux(handle, system_table, dynamic_initrds).status()
    }

    status
}
