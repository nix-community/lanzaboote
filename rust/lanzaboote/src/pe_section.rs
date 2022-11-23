use alloc::{borrow::ToOwned, string::String};

/// Extracts the data of a section of a PE file.
pub fn pe_section<'a>(file_data: &'a [u8], section_name: &str) -> Option<&'a [u8]> {
    let pe_binary = goblin::pe::PE::parse(file_data).ok()?;

    pe_binary
        .sections
        .iter()
        .find(|s| s.name().unwrap() == section_name)
        .and_then(|s| {
            let section_start: usize = s.pointer_to_raw_data.try_into().ok()?;

            assert!(s.virtual_size <= s.size_of_raw_data);
            let section_end: usize = section_start + usize::try_from(s.virtual_size).ok()?;

            Some(&file_data[section_start..section_end])
        })
}

/// Extracts the data of a section of a PE file and returns it as a string.
pub fn pe_section_as_string<'a>(file_data: &'a [u8], section_name: &str) -> Option<String> {
    pe_section(file_data, section_name)
        .and_then(|data| Some(core::str::from_utf8(data).unwrap().to_owned()))
}
