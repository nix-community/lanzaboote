#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use alloc::vec::Vec;
use core::ops::Deref;
use log::debug;
use uefi::{
    prelude::*,
    proto::{
        console::text::Output,
        device_path::DevicePath,
        loaded_image::LoadedImage,
        media::{
            file::{Directory, File, FileAttribute, FileMode, RegularFile},
            fs::SimpleFileSystem,
        },
    },
    table::boot::{OpenProtocolAttributes, OpenProtocolParams},
    Result,
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
"
        ))
        .unwrap();
}

// Find the root directory of the given image.
//
// # Safety
//
// TODO Need to find out whether we can open the protocols in safe code.
unsafe fn root_directory(image: Handle, boot_services: &BootServices) -> Result<Directory> {
    let loaded_image = boot_services.open_protocol::<LoadedImage>(
        OpenProtocolParams {
            handle: image,
            agent: image,
            controller: None,
        },
        OpenProtocolAttributes::Exclusive,
    )?;

    let device_handle = loaded_image.device();

    let device_path = boot_services.open_protocol::<DevicePath>(
        OpenProtocolParams {
            handle: device_handle,
            agent: image,
            controller: None,
        },
        OpenProtocolAttributes::Exclusive,
    )?;

    let mut device_path: &DevicePath = device_path.deref();
    let fs_handle = boot_services.locate_device_path::<SimpleFileSystem>(&mut device_path)?;

    let mut file_system = boot_services.open_protocol::<SimpleFileSystem>(
        OpenProtocolParams {
            handle: fs_handle,
            agent: image,
            controller: None,
        },
        OpenProtocolAttributes::Exclusive,
    )?;

    file_system.open_volume()
}

fn read_all(image: &mut RegularFile) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    // TODO Can we do this nicer?
    loop {
        let mut chunk = [0; 512];
        let read_bytes = image.read(&mut chunk).map_err(|e| e.status())?;

        if read_bytes == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[0..read_bytes]);
    }

    Ok(buf)
}

#[entry]
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    print_logo(system_table.stdout());

    let boot_services = system_table.boot_services();
    let mut root = unsafe { root_directory(handle, boot_services).unwrap() };

    debug!("Found root");

    let mut file = root
        .open(cstr16!("linux.efi"), FileMode::Read, FileAttribute::empty())
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

    boot_services.start_image(kernel_image).unwrap();

    Status::SUCCESS
}
