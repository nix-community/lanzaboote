use alloc::vec::Vec;
use uefi::{prelude::*, CStr16, CString16, Result};

use crate::linux_loader::InitrdLoader;
use crate::pe_loader::Image;
use crate::pe_section::pe_section_as_string;

/// Extract a string, stored as UTF-8, from a PE section.
pub fn extract_string(pe_data: &[u8], section: &str) -> Result<CString16> {
    let string = pe_section_as_string(pe_data, section).ok_or(Status::INVALID_PARAMETER)?;

    Ok(CString16::try_from(string.as_str()).map_err(|_| Status::INVALID_PARAMETER)?)
}

/// Boot the Linux kernel without checking the PE signature.
///
/// We assume that the caller has made sure that the image is safe to
/// be loaded using other means.
pub fn boot_linux_unchecked(
    handle: Handle,
    system_table: SystemTable<Boot>,
    kernel_data: Vec<u8>,
    kernel_cmdline: &CStr16,
    initrd_data: Vec<u8>,
) -> uefi::Result<()> {
    let kernel =
        Image::load(system_table.boot_services(), &kernel_data).expect("Failed to load the kernel");

    let mut initrd_loader = InitrdLoader::new(system_table.boot_services(), handle, initrd_data)?;

    let status = unsafe { kernel.start(handle, &system_table, kernel_cmdline) };

    initrd_loader.uninstall(system_table.boot_services())?;
    status.to_result()
}
