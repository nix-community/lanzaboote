use alloc::{borrow::ToOwned, vec::Vec};
use core::{cmp::min, ffi::c_void};

use goblin::pe::{PE, options::ParseOptions, section_table::SectionTable};
use uefi::{
    Result, boot,
    proto::{
        device_path::{DevicePath, FfiDevicePath},
        loaded_image::LoadedImage,
    },
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
    unsafe fn as_slice(&self) -> &'static [u8] {
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

/// An analyzed PE
pub struct ParsedPe<'a> {
    data: &'a [u8],
    parsed: PE<'a>,
}

impl<'a> ParsedPe<'a> {
    /// Parse a slice of data into a goblin PE structure.
    ///
    /// In-memory PE binaries need to be parsed differently from those read from disk.
    pub fn from_pe_in_memory(pe_in_memory: &PeInMemory) -> uefi::Result<Self> {
        // SAFETY: We get a slice that represents our currently running
        // image and then parse the PE data structures from it. This is
        // safe, because we don't touch any data in the data sections that
        // might conceivably change while we look at the slice.
        // (data sections := all unified sections that can be measured.)
        let data = unsafe { pe_in_memory.as_slice() };

        let mut parse_options = ParseOptions::default();
        // Don't parse the certificates because they are not present in the in-memory representation.
        parse_options.parse_attribute_certificates = false;
        let parsed = goblin::pe::PE::parse_with_opts(data, &parse_options)
            .map_err(|_| uefi::Status::INVALID_PARAMETER)?;

        Ok(Self { data, parsed })
    }

    /// Extracts the data of a section of a loaded PE file based on the section name.
    pub fn section_data(&self, section_name: &str) -> Option<Vec<u8>> {
        self.parsed
            .sections
            .iter()
            .find(|s| s.name().map(|n| n == section_name).unwrap_or(false))
            .and_then(|s| read_data_from_section_table(self.data, s))
    }

    /// Iterator over all section names of the PE.
    pub fn sections(&self) -> impl IntoIterator<Item = &str> {
        self.parsed.sections.iter().filter_map(|s| s.name().ok())
    }
}

/// Extracts the data of a section in a loaded PE file based on the section table.
fn read_data_from_section_table(pe_data: &[u8], section: &SectionTable) -> Option<Vec<u8>> {
    let section_start: usize = section.virtual_address.try_into().ok()?;

    // virtual_size can be larger than size_of_raw_data when
    // zero-padding is required. virtual_size can also be smaller due
    // to alignment requirements in the file.
    let section_data_end: usize = section_start
        + usize::try_from(min(section.virtual_size, section.size_of_raw_data)).ok()?;

    let mut section_data = pe_data[section_start..section_data_end].to_owned();
    section_data.resize(section.virtual_size.try_into().ok()?, 0);

    Some(section_data)
}
