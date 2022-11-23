use alloc::vec::Vec;
use uefi::{
    prelude::BootServices,
    proto::{
        device_path::text::{AllowShortcuts, DevicePathToText, DisplayOnly},
        loaded_image::LoadedImage,
        media::file::{File, FileAttribute, FileMode, RegularFile},
    },
    Result, Status,
};

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

/// Open the currently executing image as a file.
pub fn booted_image_file(boot_services: &BootServices) -> Result<RegularFile> {
    let mut file_system = boot_services.get_image_file_system(boot_services.image_handle())?;

    // The root directory of the volume where our binary lies.
    let mut root = file_system.open_volume()?;

    let loaded_image =
        boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;

    let file_path = loaded_image.file_path().ok_or(Status::NOT_FOUND)?;

    let to_text = boot_services.open_protocol_exclusive::<DevicePathToText>(
        boot_services.get_handle_for_protocol::<DevicePathToText>()?,
    )?;

    let file_path = to_text.convert_device_path_to_text(
        boot_services,
        file_path,
        DisplayOnly(false),
        AllowShortcuts(false),
    )?;

    let our_image = root.open(&file_path, FileMode::Read, FileAttribute::empty())?;

    Ok(our_image
        .into_regular_file()
        .ok_or(Status::INVALID_PARAMETER)?)
}
