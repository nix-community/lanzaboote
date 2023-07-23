use alloc::vec::Vec;
use uefi::{prelude::*, CString16, Result};

use crate::common::{boot_linux_unchecked, extract_string};
use linux_bootloader::pe_section::pe_section;
use linux_bootloader::uefi_helpers::booted_image_file;

/// Extract bytes from a PE section.
pub fn extract_bytes(pe_data: &[u8], section: &str) -> Result<Vec<u8>> {
    let bytes: Vec<u8> = pe_section(pe_data, section)
        .ok_or(Status::INVALID_PARAMETER)?
        .try_into()
        .map_err(|_| Status::INVALID_PARAMETER)?;

    Ok(bytes)
}

/// The configuration that is embedded at build time.
///
/// After this stub is built, configuration need to be embedded into the binary by adding PE
/// sections. This struct represents that information.
struct EmbeddedConfiguration {
    /// The kernel command-line.
    cmdline: CString16,

    /// The kernel as raw bytes.
    kernel: Vec<u8>,

    /// The initrd as raw bytes.
    initrd: Vec<u8>,
}

impl EmbeddedConfiguration {
    fn new(file_data: &[u8]) -> Result<Self> {
        Ok(Self {
            kernel: extract_bytes(file_data, ".linux")?,
            initrd: extract_bytes(file_data, ".initrd")?,
            cmdline: extract_string(file_data, ".cmdline")?,
        })
    }
}

pub fn boot_linux(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    // SAFETY: We get a slice that represents our currently running
    // image and then parse the PE data structures from it. This is
    // safe, because we don't touch any data in the data sections that
    // might conceivably change while we look at the slice.
    let config = unsafe {
        EmbeddedConfiguration::new(
            booted_image_file(system_table.boot_services())
                .unwrap()
                .as_slice(),
        )
        .expect("Failed to extract configuration from binary.")
    };

    boot_linux_unchecked(
        handle,
        system_table,
        config.kernel,
        &config.cmdline,
        config.initrd,
    )
    .status()
}
