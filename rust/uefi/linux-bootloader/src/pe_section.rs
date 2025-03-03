// Clippy doesn't like the lifetimes, but rustc wants them. 🤷
#![allow(clippy::needless_lifetimes)]
// Clippy doesn't understand that we exit with ? from the closure in
// and_then below and this can't be expressed with map.
#![allow(clippy::bind_instead_of_map)]

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::cmp::min;
use goblin::pe::section_table::SectionTable;

/// Extracts the data of a section in a loaded PE file
/// based on the section table.
pub fn pe_section_data<'a>(pe_data: &'a [u8], section: &SectionTable) -> Option<Vec<u8>> {
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

/// Extracts the data of a section of a loaded PE file
/// based on the section name.
pub fn pe_section<'a>(pe_data: &'a [u8], section_name: &str) -> Option<Vec<u8>> {
    let pe_binary = goblin::pe::PE::parse(pe_data).ok()?;

    pe_binary
        .sections
        .iter()
        .find(|s| s.name().map(|n| n == section_name).unwrap_or(false))
        .and_then(|s| pe_section_data(pe_data, s))
}

/// Extracts the data of a section of a loaded PE image and returns it as a string.
pub fn pe_section_as_string<'a>(pe_data: &'a [u8], section_name: &str) -> Option<String> {
    pe_section(pe_data, section_name).and_then(|data| String::from_utf8(data).ok())
}
