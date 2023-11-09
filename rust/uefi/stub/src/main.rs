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
use linux_bootloader::efivars::{export_efi_variables, get_loader_features, EfiLoaderFeatures};
use linux_bootloader::measure::measure_image;
use linux_bootloader::tpm::tpm_available;
use linux_bootloader::uefi_helpers::booted_image_file;
use log::info;
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
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    print_logo();

    if tpm_available(system_table.boot_services()) {
        info!("TPM available, will proceed to measurements.");
        // Iterate over unified sections and measure them
        // For now, ignore failures during measurements.
        // TODO: in the future, devise a threat model where this can fail
        // and ensure this hard-fail correctly.
        let _ = measure_image(
            &system_table,
            booted_image_file(system_table.boot_services()).unwrap(),
        );
        // TODO: Measure kernel parameters
        // TODO: Measure sysexts
    }

    if let Ok(features) = get_loader_features(system_table.runtime_services()) {
        if !features.contains(EfiLoaderFeatures::RandomSeed) {
            // FIXME: process random seed then on the disk.
            info!("Random seed is available, but lanzaboote does not support it yet.");
        }
    }
    export_efi_variables(STUB_NAME, &system_table).expect("Failed to export stub EFI variables");

    let status;
    // A list of dynamically assembled initrds, e.g. credential initrds or system extension
    // initrds.
    let mut dynamic_initrds: Vec<Vec<u8>> = Vec::new();

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
