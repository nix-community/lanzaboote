//! Load file support protocols.

use alloc::vec::Vec;

use log::warn;
use uefi::proto::device_path::{FfiDevicePath, DevicePath};
use uefi::proto::unsafe_protocol;
use uefi::{Result, Status};
use core::ffi::c_void;
use core::ptr;

/// The UEFI LoadFile protocol.
///
/// This protocol has a single method to load a file according to some
/// device path.
///
/// This interface is (much more) implemented by many devices, e.g. network and filesystems.
#[derive(Debug)]
#[repr(C)]
#[unsafe_protocol("56ec3091-954c-11d2-8e3f-00a0c969723b")]
pub struct LoadFileProtocol {
    load_file: unsafe extern "efiapi" fn(
        this: &mut LoadFileProtocol,
        file_path: *const FfiDevicePath,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,
}

impl LoadFileProtocol {
    /// Load file addressed by provided device path
    pub fn load_file(&mut self,
        file_path: &DevicePath,
        boot_policy: bool,
        buffer_size: &mut usize,
        buffer: *mut c_void
    ) -> Status {
        unsafe {
            (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                buffer_size as *mut usize,
                buffer
            )
        }
    }

    /// Load file addressed by the provided device path.
    pub fn load_file_in_heap(&mut self,
        file_path: &DevicePath,
        boot_policy: bool,
    ) -> Result<Vec<u8>> {
        let mut buffer_size: usize = 0;
        let mut status: Status;
        unsafe {
            status = (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                ptr::addr_of_mut!(buffer_size),
                ptr::null_mut()
            );
        }

        warn!("size obtained: {buffer_size}");

        if status.is_error() {
            return Err(status.into());
        }

        let mut buffer: Vec<u8> = Vec::with_capacity(buffer_size);
        unsafe {
            status = (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                ptr::addr_of_mut!(buffer_size),
                buffer.as_mut_ptr() as *mut c_void
            );
        }

        if status.is_error() {
            return Err(status.into());
        }

        Ok(buffer)
    }
}

/// The UEFI LoadFile2 protocol.
///
/// This protocol has a single method to load a file according to some
/// device path.
///
/// This interface is implemented by many devices, e.g. network and filesystems.
#[derive(Debug)]
#[repr(C)]
#[unsafe_protocol("4006c0c1-fcb3-403e-996d-4a6c8724e06d")]
pub struct LoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: &mut LoadFile2Protocol,
        file_path: *const FfiDevicePath,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,
}

impl LoadFile2Protocol {
    /// Load file addressed by provided device path
    pub fn load_file(&mut self,
        file_path: &DevicePath,
        boot_policy: bool,
        buffer_size: &mut usize,
        buffer: *mut c_void
    ) -> Status {
        unsafe {
            (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                buffer_size as *mut usize,
                buffer
            )
        }
    }

    /// Load file addressed by the provided device path.
    pub fn load_file_in_heap(&mut self,
        file_path: &DevicePath,
        boot_policy: bool,
    ) -> Result<Vec<u8>> {
        let mut buffer_size: usize = 0;
        let mut status: Status;
        unsafe {
            status = (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                ptr::addr_of_mut!(buffer_size),
                ptr::null_mut()
            );
        }

        if status.is_error() {
            return Err(status.into());
        }

        let mut buffer: Vec<u8> = Vec::with_capacity(buffer_size);
        unsafe {
            status = (self.load_file)(self,
                file_path.as_ffi_ptr(),
                boot_policy,
                ptr::addr_of_mut!(buffer_size),
                buffer.as_mut_ptr() as *mut c_void
            );
        }

        if status.is_error() {
            return Err(status.into());
        }

        Ok(buffer)
    }
}
