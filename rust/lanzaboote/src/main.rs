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
    CString16, Result,
};

use crate::{
    linux_loader::InitrdLoader,
    uefi_helpers::{booted_image_cmdline, booted_image_file, read_all},
};

/// Print the startup logo on boot.
fn print_logo(output: &mut Output) -> Result<()> {
    output.clear()?;

    output.output_string(cstr16!(
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
}

/// The configuration that is embedded at build time.
///
/// After lanzaboote is built, lanzatool needs to embed configuration
/// into the binary. This struct represents that information.
struct EmbeddedConfiguration {
    /// The filename of the kernel to be booted. This filename is
    /// relative to the root of the volume that contains the
    /// lanzaboote binary.
    kernel_filename: CString16,

    /// The filename of the initrd to be passed to the kernel. See
    /// `kernel_filename` for how to interpret these filenames.
    initrd_filename: CString16,
}

impl EmbeddedConfiguration {
    fn new(file: &mut RegularFile) -> Result<Self> {
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

    print_logo(system_table.stdout()).unwrap();

    let config: EmbeddedConfiguration =
        EmbeddedConfiguration::new(&mut booted_image_file(system_table.boot_services()).unwrap())
            .expect("Failed to extract configuration from binary. Did you run lanzatool?");

    let mut kernel_file;
    let initrd = {
        let mut file_system = system_table
            .boot_services()
            .get_image_file_system(handle)
            .expect("Failed to get file system handle");
        let mut root = file_system
            .open_volume()
            .expect("Failed to find ESP root directory");

        kernel_file = root
            .open(
                &config.kernel_filename,
                FileMode::Read,
                FileAttribute::empty(),
            )
            .expect("Failed to open kernel file for reading")
            .into_regular_file()
            .expect("Kernel is not a regular file");

        root.open(
            &config.initrd_filename,
            FileMode::Read,
            FileAttribute::empty(),
        )
        .expect("Failed to open initrd for reading")
        .into_regular_file()
        .expect("Initrd is not a regular file")
    };

    let kernel_cmdline =
        booted_image_cmdline(system_table.boot_services()).expect("Failed to fetch command line");

    let kernel_handle = {
        let kernel_data =
            read_all(&mut kernel_file).expect("Failed to read kernel file into memory");

        system_table
            .boot_services()
            .load_image(
                handle,
                uefi::table::boot::LoadImageSource::FromBuffer {
                    buffer: &kernel_data,
                    file_path: None,
                },
            )
            .expect("UEFI refused to load the kernel image. It may not be signed or it may not have an EFI stub.")
    };

    let mut kernel_image = system_table
        .boot_services()
        .open_protocol_exclusive::<LoadedImage>(kernel_handle)
        .expect("Failed to open the LoadedImage protocol");

    unsafe {
        kernel_image.set_load_options(
            kernel_cmdline.as_ptr() as *const u8,
            u32::try_from(kernel_cmdline.len()).unwrap(),
        );
    }

    let mut initrd_loader = InitrdLoader::new(system_table.boot_services(), handle, initrd)
        .expect("Failed to load the initrd. It may not be there or it is not signed");
    let status = system_table
        .boot_services()
        .start_image(kernel_handle)
        .status();

    initrd_loader
        .uninstall(system_table.boot_services())
        .expect("Failed to uninstall the initrd protocols");
    status
}
