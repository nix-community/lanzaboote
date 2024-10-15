use core::ffi::c_void;
use core::ptr::NonNull;

use alloc::vec::Vec;
use goblin::pe::PE;
use uefi::{
    boot::{self, AllocateType, MemoryType},
    proto::loaded_image::LoadedImage,
    table, Handle, Status,
};

/// UEFI mandates 4 KiB pages.
const UEFI_PAGE_BITS: usize = 12;
const UEFI_PAGE_MASK: usize = (1 << UEFI_PAGE_BITS) - 1;

#[cfg(target_arch = "aarch64")]
fn make_instruction_cache_coherent(memory: &[u8]) {
    use core::arch::asm;
    // Minimum cache line size is 16 bits per the CSSIDR_EL0 format.
    // For simplicity, we issue flushes in this stride unconditionally.
    const CACHE_LINE_SIZE: usize = 16;

    // The start address gets rounded down, while the end address gets rounded up.
    // This guarantees we flush precisely every cache line touching the passed slice.
    let start_address = memory.as_ptr() as usize & CACHE_LINE_SIZE.wrapping_neg();
    let end_address = ((memory.as_ptr() as usize + memory.len() - 1) | (CACHE_LINE_SIZE - 1)) + 1;

    // Compare the ARM Architecture Reference Manual, B2.4.4.

    // Make the writes to every address in the range visible at PoU.
    for address in (start_address..end_address).step_by(CACHE_LINE_SIZE) {
        unsafe {
            // SAFETY: The addressed cache line overlaps `memory`, so it must be mapped.
            asm!("dc cvau, {address}", address = in(reg) address);
        }
    }
    unsafe {
        // SAFETY: Barriers are always safe to execute.
        asm!("dsb ish");
    }

    // Force reloading the written instructions.
    for address in (start_address..end_address).step_by(4) {
        unsafe {
            // SAFETY: The addressed cache line overlaps `memory`, so it must be mapped.
            asm!("ic ivau, {address}", address = in(reg) address);
        }
    }
    unsafe {
        // SAFETY: Barriers are always safe to execute.
        asm! {
            "dsb ish",
            "isb",
        }
    }
}

#[cfg(target_arch = "x86")]
fn make_instruction_cache_coherent(_memory: &[u8]) {
    // x86 has coherent instruction cache for legacy compatibility reasons
}

#[cfg(target_arch = "x86_64")]
fn make_instruction_cache_coherent(_memory: &[u8]) {
    // x86_64 mandates coherent instruction cache
}

pub struct Image {
    image: &'static mut [u8],
    entry: extern "efiapi" fn(Handle, Option<NonNull<c_void>>) -> Status,
}

/// Converts a length in bytes to the number of required pages.
fn bytes_to_pages(bytes: usize) -> usize {
    bytes
        .checked_add(UEFI_PAGE_MASK)
        .map(|rounded_up| rounded_up >> UEFI_PAGE_BITS)
        .unwrap_or(1 << (usize::try_from(usize::BITS).unwrap() - UEFI_PAGE_BITS))
}

impl Image {
    /// Loads and relocates a PE file.
    ///
    /// The image must be handed to [`start`] later. If this does not
    /// happen, the memory allocated for the unpacked PE binary will
    /// leak.
    pub fn load(file_data: &[u8]) -> uefi::Result<Image> {
        let pe = PE::parse(file_data).map_err(|_| Status::LOAD_ERROR)?;

        // Allocate all memory the image will need in virtual memory.
        // We follow shim here and allocate as EfiLoaderCode.
        let image = {
            let section_lengths = pe
                .sections
                .iter()
                .map(|section| {
                    section
                        .virtual_address
                        .checked_add(section.virtual_size)
                        .ok_or(Status::LOAD_ERROR)
                })
                .collect::<Result<Vec<u32>, uefi::Status>>()?;

            let length = usize::try_from(section_lengths.into_iter().max().unwrap_or(0)).unwrap();

            let base = boot::allocate_pages(
                AllocateType::AnyPages,
                MemoryType::LOADER_CODE,
                bytes_to_pages(length),
            )?;

            unsafe {
                core::ptr::write_bytes(base.as_ptr(), 0, length);
                core::slice::from_raw_parts_mut(base.as_ptr(), length)
            }
        };

        // Populate all sections in virtual memory.
        for section in &pe.sections {
            let copy_size =
                usize::try_from(u32::min(section.virtual_size, section.size_of_raw_data)).unwrap();
            let raw_start = usize::try_from(section.pointer_to_raw_data).unwrap();
            let raw_end = raw_start.checked_add(copy_size).ok_or(Status::LOAD_ERROR)?;
            let virt_start = usize::try_from(section.virtual_address).unwrap();
            let virt_end = virt_start
                .checked_add(copy_size)
                .ok_or(Status::LOAD_ERROR)?;

            if virt_end > image.len() || raw_end > file_data.len() {
                return Err(Status::LOAD_ERROR.into());
            }
            image[virt_start..virt_end].copy_from_slice(&file_data[raw_start..raw_end]);
        }

        // Image base relocations are not supported.
        if pe
            .header
            .optional_header
            .and_then(|h| *h.data_directories.get_base_relocation_table())
            .is_some()
        {
            return Err(Status::INCOMPATIBLE_VERSION.into());
        }

        // On some platforms, the instruction cache is not coherent with the data cache.
        // We don't want to execute stale icache contents instead of the code we just loaded.
        // Platform-specific flushes need to be performed to prevent this from happening.
        make_instruction_cache_coherent(image);

        if pe.entry >= image.len() {
            return Err(Status::LOAD_ERROR.into());
        }
        let entry = unsafe { core::mem::transmute(&image[pe.entry]) };

        Ok(Image { image, entry })
    }

    /// Starts a trusted loaded PE file.
    /// The caller is responsible for verifying that it trusts the PE file to uphold the invariants detailed below.
    /// If the entry point returns, the image memory is subsequently deallocated.
    ///
    /// # Safety
    /// The image is assumed to be trusted. This means:
    /// * The PE file it was loaded from must have been a completely valid EFI application of the correct architecture.
    /// * If the entry point returns, it must leave the system in a state that allows our stub to continue.
    ///   In particular:
    ///   * Only memory it either has allocated, or that belongs to the image, should have been altered.
    ///   * Memory it has not allocated should not have been freed.
    ///   * Boot services must not have been exited.
    pub unsafe fn start(self, handle: Handle, load_options: &[u8]) -> Status {
        let mut loaded_image = boot::open_protocol_exclusive::<LoadedImage>(handle)
            .expect("Failed to open the LoadedImage protocol");

        let (our_data, our_size) = loaded_image.info();
        let our_load_options = loaded_image
            .load_options_as_bytes()
            .map(|options| options.as_ptr_range());

        // It seems to be impossible to allocate custom image handles.
        // Hence, we reuse our own for the kernel.
        // The shim does the same thing.
        unsafe {
            loaded_image.set_image(
                self.image.as_ptr() as *const c_void,
                self.image.len().try_into().unwrap(),
            );
            loaded_image.set_load_options(
                load_options.as_ptr(),
                u32::try_from(load_options.len()).unwrap(),
            );
        }

        let system_table = table::system_table_raw().map(NonNull::cast);
        let status = (self.entry)(handle, system_table);

        // If the kernel has exited boot services, it must not return any more, and has full control over the entire machine.
        // If the kernel entry point returned, deallocate its image, and restore our loaded image handle.
        // If it calls Exit(), that call returns directly to systemd-boot. This unfortunately causes a resource leak.
        let image = NonNull::new(self.image.as_ptr().cast_mut()).unwrap();
        boot::free_pages(image, bytes_to_pages(self.image.len())).expect("Double free attempted");

        unsafe {
            loaded_image.set_image(our_data, our_size);
            match our_load_options {
                Some(options) => loaded_image.set_load_options(
                    options.start,
                    options.end.offset_from(options.start).try_into().unwrap(),
                ),
                None => loaded_image.set_load_options(core::ptr::null(), 0),
            }
        }

        status
    }
}
