use alloc::vec::Vec;
use uefi::{prelude::*, CString16, Result};

use crate::common::{boot_linux_unchecked, extract_string, get_cmdline, get_secure_boot_status};
use linux_bootloader::pe_section::pe_section;
use linux_bootloader::uefi_helpers::booted_image_file;

/// Extract bytes from a PE section.
pub fn extract_bytes(pe_data: &[u8], section: &str) -> Result<Vec<u8>> {
    let bytes: Vec<u8> = pe_section(pe_data, section)
        .ok_or(Status::INVALID_PARAMETER)?
        .into();

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

pub fn boot_linux(handle: Handle, dynamic_initrds: Vec<Vec<u8>>) -> Status {
    // SAFETY: We get a slice that represents our currently running
    // image and then parse the PE data structures from it. This is
    // safe, because we don't touch any data in the data sections that
    // might conceivably change while we look at the slice.
    let mut config = unsafe {
        EmbeddedConfiguration::new(booted_image_file().unwrap().as_slice())
            .expect("Failed to extract configuration from binary.")
    };

    let secure_boot_enabled = get_secure_boot_status();
    let cmdline = get_cmdline(&config.cmdline, secure_boot_enabled);

    let mut final_initrd = Vec::new();
    final_initrd.append(&mut config.initrd);

    // Correctness: dynamic initrds are supposed to be validated by caller,
    // i.e. they are system extension images or credentials
    // that are supposedly measured in TPM2.
    // Therefore, it is normal to not verify their hashes against a configuration.
    for mut extra_initrd in dynamic_initrds {
        final_initrd.append(&mut extra_initrd);
    }

    boot_linux_unchecked(handle, config.kernel, &cmdline, final_initrd).status()
}
