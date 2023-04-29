use core::ffi::c_void;

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use uefi::Guid;
use uefi::{
    prelude::{BootServices, RuntimeServices},
    proto::{loaded_image::LoadedImage, media::file::RegularFile, device_path::text::{DevicePathToText, DisplayOnly, AllowShortcuts}},
    Result, table::{runtime::{VariableVendor, VariableAttributes}, SystemTable, Boot}, CStr16, cstr16,
};

use crate::part_discovery::disk_get_part_uuid;

// systemd loader's GUID
// != systemd's GUID
// FIXME: please fix me, I hate UEFI.
// https://github.com/systemd/systemd/blob/main/src/boot/efi/util.h#L114-L121
const SD_LOADER: VariableVendor = VariableVendor(Guid::from_values(
        0x4a67b082,
        0x0a4c,
        0x41cf,
        u16::from_le_bytes([0xb6, 0xC7]),
        u64::from_le_bytes([0xb6, 0xc7, 0x44, 0x0b, 0x29, 0xbb, 0x8c, 0x4f])
    ));
// const STUB_FEATURES: ???

pub fn ensure_efi_variable<'a, F>(runtime_services: &RuntimeServices,
    name: &CStr16,
    vendor: &VariableVendor,
    attributes: VariableAttributes,
    get_fallback_value: F) -> uefi::Result
    where F: FnOnce() -> uefi::Result<&'a [u8]>
{
    // If we get a variable size, a variable already exist.
    if let Err(_) = runtime_services.get_variable_size(name, vendor) {
        runtime_services.set_variable(
            name,
            &vendor,
            attributes,
            get_fallback_value()?
        )?;
    }

    uefi::Status::SUCCESS.into()
}

/// Exports systemd-stub style EFI variables
pub fn export_efi_variables(system_table: &SystemTable<Boot>) -> Result<()> {
    let boot_services = system_table.boot_services();
    let runtime_services = system_table.runtime_services();

    let loaded_image =
        boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;
    // TODO: figure out the right variable attributes
    // LoaderDevicePartUUID
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderDevicePartUUID"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        // FIXME: eeh, can we have CString16 -> &[u8] ?
        || disk_get_part_uuid(&boot_services, loaded_image.device()).map(|c| c.to_string().as_bytes())
    );
    // LoaderImageIdentifier
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderImageIdentifier"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        || {
            if let Some(dp) = loaded_image.file_path() {
                let dp_protocol = boot_services.open_protocol_exclusive::<DevicePathToText>(
                    boot_services.get_handle_for_protocol::<DevicePathToText>()?
                )?;
                dp_protocol.convert_device_path_to_text(
                    &boot_services,
                    dp,
                    DisplayOnly(false),
                    AllowShortcuts(false)
                ).map(|ps| ps.to_string().as_bytes())
            } else {
                // FIXME: I take any advice here.
                Err(uefi::Status::UNSUPPORTED.into())
            }
        });
    // LoaderFirmwareInfo
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderFirmwareInfo"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        // FIXME: respect https://github.com/systemd/systemd/blob/main/src/boot/efi/stub.c#L117
        || Ok(format!("{} {}.{}", system_table.firmware_vendor(), system_table.firmware_revision() >> 16, system_table.firmware_revision() & 0xFFFFF).as_bytes())
    );
    // LoaderFirmwareType
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderFirmwareType"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        || Ok(format!("UEFI {}", system_table.uefi_revision().to_string()).as_bytes())
    );
    // StubInfo
    let _ = runtime_services.set_variable(
        cstr16!("StubInfo"),
        &SD_LOADER,
        // FIXME: what do we want here? I think it should be locked at that moment.
        VariableAttributes::from_bits_truncate(0x0),
        "lanzastub".as_bytes() // FIXME: add version numbers and even git revision if available.
    );

    // StubFeatures
    /*let _ = runtime_services.set_variable(
        cstr16!("StubFeatures"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        STUB_FEATURES
    );*/

    Ok(())
}

/// Read the whole file into a vector.
pub fn read_all(file: &mut RegularFile) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    loop {
        let mut chunk = [0; 512];
        let read_bytes = file.read(&mut chunk).map_err(|e| e.status())?;

        if read_bytes == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[0..read_bytes]);
    }

    Ok(buf)
}

#[derive(Debug, Clone, Copy)]
pub struct PeInMemory {
    image_base: *const c_void,
    image_size: usize,
}

impl PeInMemory {
    /// Return a reference to the currently running image.
    ///
    /// # Safety
    ///
    /// The returned slice covers the whole loaded image in which we
    /// currently execute. This means the safety guarantees of
    /// [`core::slice::from_raw_parts`] that we use in this function
    /// are only guaranteed, if we we don't mutate anything in this
    /// range. This means no modification of global variables or
    /// anything.
    pub unsafe fn as_slice(&self) -> &'static [u8] {
        unsafe { core::slice::from_raw_parts(self.image_base as *const u8, self.image_size) }
    }
}

/// Open the currently executing image as a file.
pub fn booted_image_file(boot_services: &BootServices) -> Result<PeInMemory> {
    let loaded_image =
        boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;
    let (image_base, image_size) = loaded_image.info();

    Ok(PeInMemory {
        image_base,
        image_size: usize::try_from(image_size).map_err(|_| uefi::Status::INVALID_PARAMETER)?,
    })
}
