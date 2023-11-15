use core::convert::Infallible;

use alloc::{string::String, vec::Vec};
use pio::errors::CPIOError;
use uefi::fs::{Path, PathBuf};

pub type Cpio = pio::writer::Cpio<Infallible>;
pub type Result = core::result::Result<Cpio, CPIOError<Infallible>>;

/// Given a file contents and a filename, this will create an ad-hoc CPIO archive
/// containing this single item inside.
/// It is largely similar to `pack_cpio` except that it operates on a single file that is already
/// in memory.
pub fn pack_cpio_literal(
    contents: &[u8],
    target_filename: &Path,
    target_dir_prefix: &str,
    dir_mode: u32,
    access_mode: u32,
) -> Result {
    let mut cpio = Cpio::new();

    let utf8_filename = String::from(target_filename.to_cstr16());

    cpio.pack_prefix(target_dir_prefix, dir_mode)?;
    cpio.pack_one(&utf8_filename, contents, target_dir_prefix, access_mode)?;
    cpio.pack_trailer()?;

    Ok(cpio)
}

/// Given a list of filenames and a filesystem high-level interface,
/// this will pack all those files in-memory in a CPIO archive (newc format)
/// which will decompress to the provided `target_dir_prefix`.
///
/// In the CPIO archives, only the basename is retained as a filename.
///
/// For consistency of TPM2 measurements, the `files` list will be sorted in this function.
///
/// Target directory prefix will be created with `dir_mode` access privileges,
/// files will be created with `access_mode`.
///
/// All prefixes of the target directory prefix excluding itself will be created with 555
/// permission bits.
pub fn pack_cpio(
    fs: &mut uefi::fs::FileSystem,
    mut files: Vec<PathBuf>,
    target_dir_prefix: &str,
    dir_mode: u32,
    access_mode: u32,
) -> Result {
    let mut cpio = Cpio::new();

    // Ensure consistency of the CPIO archive layout for future potential measurements via TPM2.
    files.sort();

    cpio.pack_prefix(target_dir_prefix, dir_mode)?;
    for file in files {
        let utf8_filename = String::from(
            &file
                .components()
                .last()
                .expect("Expected the filename to possess a file name!"),
        );
        let contents = fs.read(file).expect("failed to read");
        cpio.pack_one(&utf8_filename, &contents, target_dir_prefix, access_mode)?;
    }
    cpio.pack_trailer()?;

    Ok(cpio)
}
