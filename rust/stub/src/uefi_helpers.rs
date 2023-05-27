use core::ffi::c_void;

use alloc::vec::Vec;
use uefi::{
    prelude::BootServices,
    proto::{loaded_image::LoadedImage, media::file::RegularFile},
    Result,
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
