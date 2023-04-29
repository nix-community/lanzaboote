use core::ffi::c_void;

use alloc::format;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use uefi::{
    guid,
    prelude::{BootServices, RuntimeServices},
    proto::{loaded_image::LoadedImage, media::file::RegularFile, device_path::text::{DevicePathToText, DisplayOnly, AllowShortcuts}},
    Result, table::{runtime::{VariableVendor, VariableAttributes}, SystemTable, Boot}, CStr16, cstr16,
};

use crate::part_discovery::disk_get_part_uuid;
use bitflags::bitflags;

/// systemd loader's GUID
/// != systemd's GUID
/// https://github.com/systemd/systemd/blob/main/src/boot/efi/util.h#L114-L121
pub const SD_LOADER: VariableVendor = VariableVendor(guid!("4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"));

/// Lanzaboote stub name
pub static STUB_INFO_STRING: &'static str = concat!("lanzastub ", env!("CARGO_PKG_VERSION"));

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct SystemdLoaderFeatures: u64 {
       const ConfigTimeout = 1 << 0;
       const ConfigTimeoutOneShot = 1 << 1;
       const EntryDefault = 1 << 2;
       const EntryOneshot = 1 << 3;
       const BootCounting = 1 << 4;
       const XBOOTLDR = 1 << 5;
       const RandomSeed = 1 << 6;
       const LoadDriver = 1 << 7;
       const SortKey = 1 << 8;
       const SavedEntry = 1 << 9;
       const DeviceTree = 1 << 10;
    }
}

/// Get the SystemdLoaderFeatures if they do exist.
pub fn get_loader_features(runtime_services: &RuntimeServices) -> Result<SystemdLoaderFeatures> {
    if let Ok(size) = runtime_services.get_variable_size(cstr16!("LoaderFeatures"), &SD_LOADER) {
        let mut buffer = vec![0; size].into_boxed_slice();
        runtime_services.get_variable(
            cstr16!("LoaderFeatures"),
            &SD_LOADER,
            &mut buffer)?;

        return SystemdLoaderFeatures::from_bits(
            u64::from_le_bytes(
                (*buffer).try_into()
                .map_err(|_err| uefi::Status::BAD_BUFFER_SIZE)?
            ))
            .ok_or_else(|| uefi::Status::INCOMPATIBLE_VERSION.into());
    }

    Ok(Default::default())
}

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SystemdStubFeatures: u64 {
       /// Is `LoaderDevicePartUUID` loaded in UEFI variables?
       const ReportBootPartition = 1 << 0;
       /// Are credentials picked up from the boot partition?
       const PickUpCredentials = 1 << 1;
       /// Are system extensions picked up from the boot partition?
       const PickUpSysExts = 1 << 2;
       /// Are we able to measure kernel image, parameters and sysexts?
       const ThreePcrs = 1 << 3;
       /// Can we pass a random seed to the kernel?
       const RandomSeed = 1 << 4;
    }
}

/// Ensures that an UEFI variable is set or set it with a fallback value
/// computed in a lazy way.
pub fn ensure_efi_variable<'a, F>(runtime_services: &RuntimeServices,
    name: &CStr16,
    vendor: &VariableVendor,
    attributes: VariableAttributes,
    get_fallback_value: F) -> uefi::Result
    where F: FnOnce() -> uefi::Result<Vec<u8>>
{
    // If we get a variable size, a variable already exist.
    if let Err(_) = runtime_services.get_variable_size(name, vendor) {
        runtime_services.set_variable(
            name,
            &vendor,
            attributes,
            get_fallback_value()?.as_slice()
        )?;
    }

    uefi::Status::SUCCESS.into()
}

/// Exports systemd-stub style EFI variables
pub fn export_efi_variables(system_table: &SystemTable<Boot>) -> Result<()> {
    let boot_services = system_table.boot_services();
    let runtime_services = system_table.runtime_services();

    let stub_features: SystemdStubFeatures =
        SystemdStubFeatures::ReportBootPartition;

    let loaded_image =
        boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;

    let default_attributes = VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS;

    // LoaderDevicePartUUID
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderDevicePartUUID"),
        &SD_LOADER,
        default_attributes,
        || disk_get_part_uuid(&boot_services, loaded_image.device()).map(|c| c.to_string().into_bytes())
    );
    // LoaderImageIdentifier
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderImageIdentifier"),
        &SD_LOADER,
        default_attributes,
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
                ).map(|ps| ps.to_string().into_bytes())
            } else {
                // If we cannot retrieve the filepath of the loaded image
                // Then, we cannot set `LoaderImageIdentifier`.
                Err(uefi::Status::UNSUPPORTED.into())
            }
        });
    // LoaderFirmwareInfo
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderFirmwareInfo"),
        &SD_LOADER,
        default_attributes,
        || Ok(format!("{} {}.{:02}", system_table.firmware_vendor(), system_table.firmware_revision() >> 16, system_table.firmware_revision() & 0xFFFFF).into_bytes())
    );
    // LoaderFirmwareType
    let _ = ensure_efi_variable(runtime_services,
        cstr16!("LoaderFirmwareType"),
        &SD_LOADER,
        default_attributes,
        || Ok(format!("UEFI {}", system_table.uefi_revision().to_string()).into_bytes())
    );
    // StubInfo
    // FIXME: ideally, no one should be able to overwrite `StubInfo`, but that would require
    // constructing an EFI authenticated variable payload. This seems overcomplicated for now.
    let _ = runtime_services.set_variable(
        cstr16!("StubInfo"),
        &SD_LOADER,
        default_attributes,
        STUB_INFO_STRING.as_bytes()
    );

    // StubFeatures
    let _ = runtime_services.set_variable(
        cstr16!("StubFeatures"),
        &SD_LOADER,
        VariableAttributes::from_bits_truncate(0x0),
        &stub_features.bits().to_le_bytes()
    );

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
