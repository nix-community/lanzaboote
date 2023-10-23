//! This module implements the protocols to hand an initrd to the
//! Linux kernel.
//!
//! XXX The initrd signature validation is vulnerable to TOCTOU,
//! because we read the initrd multiple times. The code needs to be
//! restructured to solve this.

use core::{ffi::c_void, pin::Pin, ptr::slice_from_raw_parts_mut};

use alloc::{boxed::Box, vec::Vec};
use uefi::proto::device_path::build;
use uefi::proto::device_path::build::DevicePathBuilder;
use uefi::{
    guid,
    prelude::BootServices,
    proto::{
        device_path::{DevicePath, FfiDevicePath},
        unsafe_protocol,
    },
    Guid, Handle, Identify, Result, ResultExt, Status,
};

/// The GUID of the INITRD EFI protocol of Linux.
const LINUX_EFI_INITRD_MEDIA_GUID: Guid = guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");

/// Stores the device path that is build by [`build_linux_initrd_device_path`]
/// during runtime.
///
/// # Safety
/// - `OnceCell` is thread safe
/// - The `static` lifetime is fine as the allocated backing memory is covered
///   by the UEFI memory map. It remains valid even after exiting the boot
///   services for the entire runtime of the system.
static LINUX_INITRD_DEVICE_PATH: once_cell::race::OnceBox<&'static DevicePath> =
    once_cell::race::OnceBox::new();

/// The UEFI LoadFile2 protocol.
///
/// This protocol has a single method to load a file.
#[repr(C)]
#[unsafe_protocol("4006c0c1-fcb3-403e-996d-4a6c8724e06d")]
struct LoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: &mut LoadFile2Protocol,
        file_path: *const FfiDevicePath,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,

    // This is not part of the official protocol struct.
    initrd_data: Vec<u8>,
}

impl LoadFile2Protocol {
    fn load_file(
        &mut self,
        _file_path: Option<&FfiDevicePath>,
        _boot_policy: bool,
        buffer_size: Option<&mut usize>,
        buffer: *mut u8,
    ) -> Result<()> {
        let buffer_size = buffer_size.ok_or(uefi::Error::new(Status::INVALID_PARAMETER, ()))?;
        if buffer.is_null() || *buffer_size < self.initrd_data.len() {
            // Give the caller a hint for the right buffer size.
            *buffer_size = self.initrd_data.len();
            return Err(Status::BUFFER_TOO_SMALL.into());
        }

        let output_slice: &mut [u8] =
            unsafe { &mut *slice_from_raw_parts_mut(buffer, self.initrd_data.len()) };

        output_slice.copy_from_slice(&self.initrd_data);

        Ok(())
    }
}

unsafe extern "efiapi" fn raw_load_file(
    this: &mut LoadFile2Protocol,
    file_path: *const FfiDevicePath,
    boot_policy: bool,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> Status {
    this.load_file(
        file_path.as_ref(),
        boot_policy,
        buffer_size.as_mut(),
        buffer.cast(),
    )
    .status()
}

/// A RAII wrapper to install and uninstall the Linux initrd loading
/// protocol.
///
/// **Note:** You need to call [`InitrdLoader::uninstall`], before
/// this is dropped.
pub struct InitrdLoader {
    proto: Pin<Box<LoadFile2Protocol>>,
    handle: Handle,
    registered: bool,
}

impl InitrdLoader {
    /// Create a new [`InitrdLoader`].
    ///
    /// `handle` is the handle where the protocols are registered
    /// on. `file` is the file that is served to Linux.
    pub fn new(boot_services: &BootServices, handle: Handle, initrd_data: Vec<u8>) -> Result<Self> {
        let mut lf2_proto = Box::pin(LoadFile2Protocol {
            load_file: raw_load_file,
            initrd_data,
        });

        // Linux finds the right handle by looking for something that
        // implements the device path protocol for the specific device
        // path.
        init_linux_initrd_device_path();

        unsafe {
            let mut initrd_proto_buf = Vec::new();
            let initrd_proto = build_linux_initrd_device_path(&mut initrd_proto_buf);
            let initrd_proto_ptr = initrd_proto.as_ffi_ptr().cast_mut().cast::<c_void>();

            boot_services.install_protocol_interface(
                Some(handle),
                &DevicePath::GUID,
                initrd_proto_ptr,
            )?;

            let lf2_proto_ptr: *mut LoadFile2Protocol = lf2_proto.as_mut().get_mut();

            boot_services.install_protocol_interface(
                Some(handle),
                &LoadFile2Protocol::GUID,
                lf2_proto_ptr as *mut c_void,
            )?;
        }

        Ok(InitrdLoader {
            handle,
            proto: lf2_proto,
            registered: true,
        })
    }

    pub fn uninstall(&mut self, boot_services: &BootServices) -> Result<()> {
        // This should only be called once.
        assert!(self.registered);

        let initrd_proto_dp_ptr = LINUX_INITRD_DEVICE_PATH
            .get()
            .unwrap()
            .as_ffi_ptr()
            .cast_mut()
            .cast::<c_void>();

        unsafe {
            boot_services.uninstall_protocol_interface(
                self.handle,
                &DevicePath::GUID,
                initrd_proto_dp_ptr,
            )?;

            let lf_proto: *mut LoadFile2Protocol = self.proto.as_mut().get_mut();

            boot_services.uninstall_protocol_interface(
                self.handle,
                &LoadFile2Protocol::GUID,
                lf_proto as *mut c_void,
            )?;
        }

        self.registered = false;

        Ok(())
    }
}

/// Builds the device path for the LINUX_EFI_INITRD_MEDIA protocol.
/// It is associated with the [`LINUX_EFI_INITRD_MEDIA_GUID`].
/// The Linux kernel points us to [u-boot] for more documentation.
///
/// [u-boot]: https://github.com/u-boot/u-boot/commit/ec80b4735a593961fe701cc3a5d717d4739b0fd0#diff-1f940face4d1cf74f9d2324952759404d01ee0a81612b68afdcba6b49803bdbbR28
fn build_linux_initrd_device_path(vec_buf: &mut Vec<u8>) -> &DevicePath {
    DevicePathBuilder::with_vec(vec_buf)
        .push(&build::media::Vendor {
            vendor_guid: LINUX_EFI_INITRD_MEDIA_GUID,
            vendor_defined_data: &[],
        })
        // Unwrap is fine as the vec grows to the required size automatically.
        .unwrap()
        .finalize()
        // Unwrap is fine as the vec grows to the required size automatically.
        .unwrap()
}

/// Initializes the global static [`LINUX_INITRD_DEVICE_PATH`].
///
/// Idempotent function.
fn init_linux_initrd_device_path() {
    if LINUX_INITRD_DEVICE_PATH.get().is_some() {
        log::debug!("LINUX_INITRD_DEVICE_PATH already initialized");
    }
    let _ = LINUX_INITRD_DEVICE_PATH.get_or_init(|| {
        let mut vec = Vec::new();
        {
            let _ = build_linux_initrd_device_path(&mut vec);
        }
        let device_path = vec.leak();
        let device_path =
            unsafe { core::mem::transmute::<&mut [u8], &'static DevicePath>(device_path) };
        Box::new(device_path)
    });
}

impl Drop for InitrdLoader {
    fn drop(&mut self) {
        // Dropped without unregistering!
        assert!(!self.registered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_initrd_device_path() {
        // Extracted actual runtime path.
        let expected_device_path: [u8; 24] = [
            0x04, 0x03, 0x14, 0x00, 0x27, 0xe4, 0x68, 0x55, 0xfc, 0x68, 0x3d, 0x4f, 0xac, 0x74, 0xca, 0x55,
            0x52, 0x31, 0xcc, 0x68, 0x7f, 0xff, 0x04, 0x00,
        ];

        let mut dp_buf = Vec::new();
        let dp = build_linux_initrd_device_path(&mut dp_buf);
        assert_eq!(dp.as_bytes(), &expected_device_path);
    }
}
