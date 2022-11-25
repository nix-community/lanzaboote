#![no_main]
#![no_std]
#![feature(abi_efiapi)]
#![feature(negative_impls)]

extern crate alloc;

mod linux_loader;
mod pe_section;
mod uefi_helpers;

use pe_section::pe_section_as_string;
use uefi::{
    prelude::*,
    proto::{
        console::text::Output,
        loaded_image::LoadedImage,
        media::file::{File, FileAttribute, FileMode, RegularFile},
    },
    CString16, Error,
};

use crate::{
    linux_loader::InitrdLoader,
    uefi_helpers::{booted_image_cmdline, booted_image_file, read_all},
};

fn print_logo(output: &mut Output) {
    output.clear().unwrap();

    output
        .output_string(cstr16!(
            "
  _                      _                 _\r
 | |                    | |               | |\r
 | | __ _ _ __  ______ _| |__   ___   ___ | |_ ___\r
 | |/ _` | '_ \\|_  / _` | '_ \\ / _ \\ / _ \\| __/ _ \\\r
 | | (_| | | | |/ / (_| | |_) | (_) | (_) | ||  __/\r
 |_|\\__,_|_| |_/___\\__,_|_.__/ \\___/ \\___/ \\__\\___|\r
\r
"
        ))
        .unwrap();
}

struct EmbeddedConfiguration {
    kernel_filename: CString16,
    initrd_filename: CString16,
}

impl EmbeddedConfiguration {
    fn new(file: &mut RegularFile) -> Result<Self, Error> {
        file.set_position(0)?;
        let file_data = read_all(file)?;

        let kernel_filename =
            pe_section_as_string(&file_data, ".kernelp").ok_or(Status::INVALID_PARAMETER)?;
        let initrd_filename =
            pe_section_as_string(&file_data, ".initrdp").ok_or(Status::INVALID_PARAMETER)?;

        Ok(Self {
            kernel_filename: CString16::try_from(kernel_filename.as_str()).unwrap(),
            initrd_filename: CString16::try_from(initrd_filename.as_str()).unwrap(),
        })
    }
}

#[entry]
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    print_logo(system_table.stdout());

    let config: EmbeddedConfiguration =
        EmbeddedConfiguration::new(&mut booted_image_file(system_table.boot_services()).unwrap())
            .unwrap();

    let mut file_system = system_table
        .boot_services()
        .get_image_file_system(handle)
        .unwrap();
    let mut root = file_system.open_volume().unwrap();

    let mut kernel_file = root
        .open(
            &config.kernel_filename,
            FileMode::Read,
            FileAttribute::empty(),
        )
        .unwrap()
        .into_regular_file()
        .unwrap();

    let initrd = root
        .open(
            &config.initrd_filename,
            FileMode::Read,
            FileAttribute::empty(),
        )
        .unwrap()
        .into_regular_file()
        .unwrap();

    // We need to manually drop those to be able to touch the system_table again.
    drop(root);
    drop(file_system);

    let kernel_cmdline = booted_image_cmdline(system_table.boot_services()).unwrap();

    let kernel_data = read_all(&mut kernel_file).unwrap();
    let kernel_handle = system_table
        .boot_services()
        .load_image(
            handle,
            uefi::table::boot::LoadImageSource::FromBuffer {
                buffer: &kernel_data,
                file_path: None,
            },
        )
        .unwrap();

    let mut kernel_image = system_table
        .boot_services()
        .open_protocol_exclusive::<LoadedImage>(kernel_handle)
        .unwrap();

    unsafe {
        kernel_image.set_load_options(
            kernel_cmdline.as_ptr() as *const u8,
            u32::try_from(kernel_cmdline.len()).unwrap(),
        );
    }

    let mut initrd_loader =
        InitrdLoader::new(system_table.boot_services(), handle, initrd).unwrap();
    let status = system_table
        .boot_services()
        .start_image(kernel_handle)
        .status();

    initrd_loader
        .uninstall(system_table.boot_services())
        .unwrap();
    status
}
