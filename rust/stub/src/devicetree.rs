use alloc::boxed::Box;
use uefi::{Handle, proto::{unsafe_protocol, media::file::{FileHandle, Directory, File, FileInfo}}, prelude::BootServices, CStr16, data_types::PhysicalAddress, table::boot::PAGE_SIZE};
use core::{ffi::c_void};
use alloc::vec::Vec;
use bitflags::bitflags;

// TODO:
// - implement cleanup (Drop)
// - cleanup allocate
// - cleanup PhysicalAddress conversions / *mut c_void
// - clarify SAFETY for u32/usize and the copies

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct DeviceTreeFixupFlags: u32 {
        /// Add nodes and update properties
        const ApplyFixups = 0x1;
        /// Reserve memory according to the /reserved-memory node
        /// and the memory reservation block
        const ReserveMemory = 0x2;
    }
}

impl Default for DeviceTreeFixupFlags {
    fn default() -> Self {
        Self::ApplyFixups | Self::ReserveMemory
    }
}


/// The UEFI DeviceTreeFixup protocol
///
/// This protocol has a single method to fixup the devicetree.
/// https://github.com/U-Boot-EFI/EFI_DT_FIXUP_PROTOCOL
#[repr(C)]
#[unsafe_protocol("e617d64c-fe08-46da-f4dc-bbd5870c7300")]
struct DeviceTreeFixupProtocol {
    revision: u64,
    /// Applies fix-ups to a devicetree, makes memory reservations,
    /// installs the device-tree as a configuration table.
    fixup: unsafe extern "efiapi" fn(
        this: &mut DeviceTreeFixupProtocol,
        devicetree_buffer: *const c_void,
        devicetree_buffer_size: *mut usize,
        flags: DeviceTreeFixupFlags
    ) -> uefi::Status
}

/// File DeviceTree version 1 "minimal" size
const FDT_V1_SIZE: u64 = 7 * 4;

struct DeviceTree {
    current: *const c_void,
    pages: usize
}

fn div_round_up(n: usize, divisor: usize) -> usize {
    debug_assert!(divisor != 0, "Division by zero!");
    if n == 0 {
        0
    } else {
        (n - 1) / divisor + 1
    }
}

impl DeviceTree {
    pub fn new() {

    }

    fn cleanup(&mut self) {
    }

    fn allocate(&mut self, bs: &BootServices, size: usize) -> uefi::Result<*mut c_void> {
        let pages: usize = div_round_up(size, PAGE_SIZE);

        let addr = bs.allocate_pages(
            uefi::table::boot::AllocateType::AnyPages,
            uefi::table::boot::MemoryType::ACPI_RECLAIM,
            pages)?;

        self.pages = pages;
        self.current = addr;

        Ok(addr)
    }

    fn allocated_size(&self) -> usize {
        self.pages * PAGE_SIZE
    }

    fn perform_fixup(&mut self, bs: &BootServices, buffer_size: usize) -> uefi::Result {
        if let Ok(fixup_handle) = bs.get_handle_for_protocol::<DeviceTreeFixupProtocol>() {
            let mut fixup_protocol = bs.open_protocol_exclusive::<DeviceTreeFixupProtocol>(fixup_handle)?;
            let mut size = self.allocated_size();
            let mut status = (fixup_protocol.fixup)(&mut fixup_protocol, self.current, &mut size, Default::default());
            if status == uefi::Status::BUFFER_TOO_SMALL {
                let oldptr = self.current;
                let oldpages = self.pages;

                let newptr = self.allocate(bs, size)?;

                // SAFETY:
                // `oldptr` is a previously valid allocated pointer of buffer_size
                // `newptr` is a newly allocated pointer of size `size` > `buffer_size`
                // `oldptr` and `newptr` are aligned by virtue of allocations properties
                // `oldptr` and `newptr` cannot overlap because `dst` has not been freed yet
                // and allocator won't give the same memory mapping again.
                unsafe { core::ptr::copy_nonoverlapping(oldptr, newptr, buffer_size); }

                size = self.allocated_size();
                status = (fixup_protocol.fixup)(&mut fixup_protocol, self.current, &mut size, Default::default());

                status.into()
            } else {
                status.into()
            }
        } else {
            uefi::Status::UNSUPPORTED.into()
        }
    }

    pub fn install(&mut self, bs: &BootServices, root_dir: &mut Directory, name: &CStr16) -> uefi::Result {
        let mut file_hnd = root_dir.open(name, uefi::proto::media::file::FileMode::Read, uefi::proto::media::file::FileAttribute::READ_ONLY)?;
        let file_info: Box<FileInfo> = file_hnd.get_boxed_info()?;

        if file_hnd.is_directory()? || file_info.file_size() < FDT_V1_SIZE || file_info.file_size() > 32 * 1024 * 1024 {
            // 32MiB device tree blob is a limit that was set by systemd-stub
            // We copy it here.
            return uefi::Status::INVALID_PARAMETER.into();
        }

        // TODO: self.original = find_configuration_table();

        // SAFETY: if usize < u64, that's bad.
        let mut buffer = self.allocate(bs, file_info.file_size() as usize)?;
        // SAFETY: please check me, I'm not sure of myself.
        unsafe { file_hnd.into_regular_file().unwrap().read(*buffer.cast::<&mut [u8]>()); }

        // SAFETY: if usize < u64, that's bad.
        self.perform_fixup(bs, file_info.file_size() as usize)?;

        // TODO: install configuration table
        Ok(())
    }

    pub fn install_from_memory(&mut self, bs: &BootServices, dtb_buffer: &[u8]) -> uefi::Result {
        // TODO: self.original = find_configuration_table();
        let mut buffer = self.allocate(bs, dtb_buffer.len())?;
        // SAFETY: ...
        unsafe { core::ptr::copy_nonoverlapping(dtb_buffer, buffer, dtb_buffer.len()); }

        self.perform_fixup(bs, dtb_buffer.len())?;

        // TODO: install configuration table
        Ok(())
    }
}
