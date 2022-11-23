#![no_main]
#![no_std]
#![feature(abi_efiapi)]
#![feature(negative_impls)]

extern crate alloc;

mod linux_loader;
mod pe_section;
mod uefi_helpers;

use log::{debug, info};
use uefi::{
    prelude::*,
    proto::{
        console::text::Output,
        device_path::text::{AllowShortcuts, DevicePathToText, DisplayOnly},
        loaded_image::LoadedImage,
        media::file::{File, FileAttribute, FileMode, RegularFile},
    },
    table::boot::{OpenProtocolAttributes, OpenProtocolParams},
    Result,
};

use crate::{
    linux_loader::InitrdLoader,
    pe_section::pe_section,
    uefi_helpers::{booted_image_file, read_all},
};

fn print_logo(output: &mut Output) {
    output.clear().unwrap();

    output
        .output_string(cstr16!(
            "
  _                      _                 _   \r
 | |                    | |               | |  \r
 | | __ _ _ __  ______ _| |__   ___   ___ | |_ \r
 | |/ _` | '_ \\|_  / _` | '_ \\ / _ \\ / _ \\| __|\r
 | | (_| | | | |/ / (_| | |_) | (_) | (_) | |_ \r
 |_|\\__,_|_| |_/___\\__,_|_.__/ \\___/ \\___/ \\__|\r
\r
"
        ))
        .unwrap();
}

#[entry]
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    print_logo(system_table.stdout());

    let boot_services = system_table.boot_services();

    {
        let image_data = read_all(&mut booted_image_file(boot_services).unwrap()).unwrap();

        if let Some(data) = pe_section(&image_data, ".osrel") {
            info!("osrel = {}", core::str::from_utf8(data).unwrap_or("???"))
        }
    }

    let mut file_system = boot_services.get_image_file_system(handle).unwrap();
    let mut root = file_system.open_volume().unwrap();

    let mut file = root
        .open(cstr16!("linux.efi"), FileMode::Read, FileAttribute::empty())
        .unwrap()
        .into_regular_file()
        .unwrap();

    let initrd = root
        .open(cstr16!("initrd"), FileMode::Read, FileAttribute::empty())
        .unwrap()
        .into_regular_file()
        .unwrap();

    debug!("Opened file");

    let kernel = read_all(&mut file).unwrap();

    let kernel_image = boot_services
        .load_image(
            handle,
            uefi::table::boot::LoadImageSource::FromBuffer {
                buffer: &kernel,
                file_path: None,
            },
        )
        .unwrap();

    let mut initrd_loader = InitrdLoader::new(&boot_services, handle, initrd).unwrap();
    let status = boot_services.start_image(kernel_image).status();

    initrd_loader.uninstall(&boot_services).unwrap();
    status
}
