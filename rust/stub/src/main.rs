#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod common;
mod efivars;
mod linux_loader;
mod measure;
mod pe_loader;
mod pe_section;
mod tpm;
mod uefi_helpers;
mod unified_sections;

#[cfg(feature = "fat")]
mod fat;

#[cfg(feature = "thin")]
mod thin;

#[cfg(all(feature = "fat", feature = "thin"))]
compile_error!("A thin and fat stub cannot be produced at the same time, disable either `thin` or `fat` feature");

use efivars::{export_efi_variables, get_loader_features, EfiLoaderFeatures};
use log::info;
use measure::measure_image;
use tpm::tpm_available;
use uefi::prelude::*;

use crate::uefi_helpers::booted_image_file;

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
        unsafe {
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
    }

    if let Ok(features) = get_loader_features(system_table.runtime_services()) {
        if !features.contains(EfiLoaderFeatures::RandomSeed) {
            // FIXME: process random seed then on the disk.
            info!("Random seed is available, but lanzaboote does not support it yet.");
        }
    }
    export_efi_variables(&system_table).expect("Failed to export stub EFI variables");

    let status;

    #[cfg(feature = "fat")]
    {
        status = fat::boot_linux(handle, system_table)
    }

    #[cfg(feature = "thin")]
    {
        status = thin::boot_linux(handle, system_table)
    }

    status
}
