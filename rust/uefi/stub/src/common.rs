use alloc::vec::Vec;
use log::warn;
use uefi::{
    guid, prelude::*, proto::loaded_image::LoadedImage, table::runtime::VariableVendor, CStr16,
    CString16, Result,
};

use linux_bootloader::linux_loader::InitrdLoader;
use linux_bootloader::pe_loader::Image;
use linux_bootloader::pe_section::pe_section_as_string;

/// Extract a string, stored as UTF-8, from a PE section.
pub fn extract_string(pe_data: &[u8], section: &str) -> Result<CString16> {
    let string = pe_section_as_string(pe_data, section).ok_or(Status::INVALID_PARAMETER)?;

    Ok(CString16::try_from(string.as_str()).map_err(|_| Status::INVALID_PARAMETER)?)
}

/// Obtain the kernel command line that should be used for booting.
///
/// If Secure Boot is active, this is always the embedded one (since the one passed from the bootloader may come from a malicious type 1 entry).
/// If Secure Boot is not active, the command line passed from the bootloader is used, falling back to the embedded one.
pub fn get_cmdline(
    embedded: &CStr16,
    boot_services: &BootServices,
    secure_boot_enabled: bool,
) -> Vec<u8> {
    if secure_boot_enabled {
        // The command line passed from the bootloader cannot be trusted, so it is not used when Secure Boot is active.
        embedded.as_bytes().to_vec()
    } else {
        let passed = boot_services
            .open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())
            .map(|loaded_image| loaded_image.load_options_as_bytes().map(|b| b.to_vec()));
        match passed {
            Ok(Some(passed)) => passed,
            // If anything went wrong, fall back to the embedded command line.
            _ => embedded.as_bytes().to_vec(),
        }
    }
}

/// Check whether Secure Boot is active, and we should be enforcing integrity checks.
///
/// In case of doubt, true is returned to be on the safe side.
pub fn get_secure_boot_status(runtime_services: &RuntimeServices) -> bool {
    // The firmware initialized SecureBoot to 1 if performing signature checks, and 0 if it doesn't.
    // Applications are not supposed to modify this variable (in particular, don't change the value from 1 to 0).
    let mut secure_boot_value = [1];
    let secure_boot_enabled = runtime_services
        .get_variable(
            cstr16!("SecureBoot"),
            &VariableVendor(guid!("8be4df61-93ca-11d2-aa0d-00e098032b8c")),
            &mut secure_boot_value,
        )
        .and_then(|v| {
            if !v.0.is_empty() {
                // Be defensive and treat everything that is not 0 as enabled.
                Ok(v.0[0] != 0)
            } else {
                Err(uefi::Error::from(Status::BAD_BUFFER_SIZE))
            }
        })
        .or_else(|e| -> Result<bool, ()> {
            if e.status() == Status::NOT_FOUND {
                warn!("SecureBoot variable not found. Assuming Secure Boot is disabled.");
                Ok(false)
            } else {
                warn!("Failed reading SecureBoot variable: {e}! Assuming it's enabled.");
                Ok(true)
            }
        })
        // Safe, because we map all Err to Ok above. Once ! is stabilized, we can replace () with ! above.
        .unwrap();

    if !secure_boot_enabled {
        warn!("Secure Boot is not active!");
    }

    secure_boot_enabled
}

/// Boot the Linux kernel without checking the PE signature.
///
/// We assume that the caller has made sure that the image is safe to
/// be loaded using other means.
pub fn boot_linux_unchecked(
    handle: Handle,
    system_table: SystemTable<Boot>,
    kernel_data: Vec<u8>,
    kernel_cmdline: &[u8],
    initrd_data: Vec<u8>,
) -> uefi::Result<()> {
    let kernel =
        Image::load(system_table.boot_services(), &kernel_data).expect("Failed to load the kernel");

    let mut initrd_loader = InitrdLoader::new(system_table.boot_services(), handle, initrd_data)?;

    let status = unsafe { kernel.start(handle, &system_table, kernel_cmdline) };

    initrd_loader.uninstall(system_table.boot_services())?;
    status.to_result()
}
