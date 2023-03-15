// Clippy doesn't like the lifetimes, but rustc wants them. 🤷
#![allow(clippy::needless_lifetimes)]
// Clippy doesn't understand that we exit with ? from the closure in
// and_then below and this can't be expressed with map.
#![allow(clippy::bind_instead_of_map)]

use alloc::{borrow::ToOwned, string::String};

/// Extracts the data of a section of a loaded PE file.
pub fn pe_section<'a>(pe_data: &'a [u8], section_name: &str) -> Option<&'a [u8]> {
    let pe_binary = goblin::pe::PE::parse(pe_data).ok()?;

    pe_binary
        .sections
        .iter()
        .find(|s| s.name().map(|n| n == section_name).unwrap_or(false))
        .and_then(|s| {
            let section_start: usize = s.virtual_address.try_into().ok()?;

            assert!(s.virtual_size <= s.size_of_raw_data);
            let section_end: usize = section_start + usize::try_from(s.virtual_size).ok()?;

            Some(&pe_data[section_start..section_end])
        })
}

/// Extracts the data of a section of a loaded PE image and returns it as a string.
pub fn pe_section_as_string<'a>(pe_data: &'a [u8], section_name: &str) -> Option<String> {
    pe_section(pe_data, section_name).map(|data| core::str::from_utf8(data).unwrap().to_owned())
}
