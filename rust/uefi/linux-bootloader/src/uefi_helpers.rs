use core::ffi::c_void;

use uefi::{
    boot,
    proto::{
        device_path::{DevicePath, FfiDevicePath},
        loaded_image::LoadedImage,
    },
    Result,
};

#[derive(Debug, Clone, Copy)]
pub struct PeInMemory {
    image_device_path: Option<*const FfiDevicePath>,
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

    /// Return optionally a reference to the device path
    /// relative to this image's simple file system.
    pub fn file_path(&self) -> Option<&DevicePath> {
        // SAFETY:
        //
        // The returned reference to the device path will be alive as long
        // as `self` is alive as it relies on the thin internal pointer to remain around,
        // which is guaranteed as long as the structure is not dropped.
        //
        // This means that the safety guarantees of [`uefi::device_path::DevicePath::from_ffi_ptr`]
        // are guaranteed.
        unsafe {
            self.image_device_path
                .map(|ptr| DevicePath::from_ffi_ptr(ptr))
        }
    }
}

/// Open the currently executing image as a file.
pub fn booted_image_file() -> Result<PeInMemory> {
    let loaded_image = boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle())?;
    let (image_base, image_size) = loaded_image.info();

    Ok(PeInMemory {
        image_device_path: loaded_image.file_path().map(|dp| dp.as_ffi_ptr()),
        image_base,
        image_size: usize::try_from(image_size).map_err(|_| uefi::Status::INVALID_PARAMETER)?,
    })
}
