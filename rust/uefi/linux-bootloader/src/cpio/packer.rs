use alloc::vec::Vec;
use uefi::{CStr16, CString16};

use super::writer::Cpio;

pub fn pack_cpio(
    fs: &mut uefi::fs::FileSystem,
    mut files: Vec<CString16>,
    target_dir_prefix: &str,
    dir_mode: u32,
    access_mode: u32) -> uefi::fs::FileSystemResult<Cpio> {
    // Ensure uniform and stability to make TPM measurements independent of the read order.
    files.sort();

    let mut cpio = Cpio::new();
    cpio.pack_prefix(target_dir_prefix, dir_mode).expect("Failed to pack the prefix.");
    for filename in files {
        let contents = fs.read(filename.as_ref())?;
        cpio.pack_one(filename.as_ref(), &contents, target_dir_prefix, access_mode).expect("Failed to pack an element.");
    }

    cpio.pack_trailer().expect("Failed to pack the trailer.");

    Ok(cpio)
}

pub fn pack_cpio_literal(
    data: &Vec<u8>,
    target_dir_prefix: &str,
    target_filename: &CStr16,
    dir_mode: u32,
    access_mode: u32) -> uefi::Result<Cpio> {
    let mut cpio = Cpio::new();

    cpio.pack_prefix(target_dir_prefix, dir_mode)?;
    cpio.pack_one(
        target_filename,
        data,
        target_dir_prefix,
        access_mode)?;
    cpio.pack_trailer()?;

    Ok(cpio)
}
