use uefi::{CStr16, proto::{loaded_image::LoadedImage, tcg::PcrIndex, media::fs::SimpleFileSystem}, CString16, prelude::BootServices};
use alloc::{vec::Vec, string::String};
use acid_io::{byteorder::WriteBytesExt, {Cursor, Write}, Result};

use crate::tpm::tpm_log_event_ascii;

const MAGIC_NUMBER: &[u8; _] = b"070701";
const TRAILER_NAME: &str= "TRAILER!!!";
const CPIO_HEX: &[u8; _] = "0123456789abcdef";

struct Entry {
    name: String,
    ino: u32,
    mode: u32,
    uid: u32,
    gid: u32,
    nlink: u32,
    mtime: u32,
    file_size: u32,
    dev_major: u32,
    dev_minor: u32,
    rdev_major: u32,
    rdev_minor: u32,
}

const STATIC_HEADER_LEN: usize = core::mem::size_of::<Entry>()
    - core::mem::size_of::<String>() // remove `name` size, which cannot be derived statically
    + core::mem::size_of_val(MAGIC_NUMBER)
    + core::mem::size_of::<usize>() // filename size
    + 1                             // NULL-terminator for filename (\0)
    + core::mem::size_of::<u32>(); // CRC

/// Compute the necessary padding based on the provided length
/// It returns None if no padding is necessary.
fn compute_pad4(len: usize) -> Option<Vec<u8>> {
    let overhang = len % 4;
    if overhang != 0 {
        let repeat = 4 - overhang;
        Some(vec![0u8; repeat])
    } else {
        None
    }
}

trait WriteBytesExt : Write {
    fn write_cpio_word(&mut self, word: u32) -> Result<(), acid_io::Error> {
        // A CPIO word is the hex(word) written as chars.
        // We do it manually because format! will allocate.
        self.write_all(
            word.to_le_bytes()
            .into_iter()
            .enumerate()
            .map(|(i, c)| CPIO_HEX[(c >> (4 * i)) & 0xF])
            .rev()
        )
    }

    fn write_cpio_header(&mut self, entry: Entry) -> Result<usize, acid_io::Error> {
        let mut header_size = STATIC_HEADER_LEN;
        self.write_cpio_word(MAGIC_NUMBER)?;
        self.write_cpio_word(entry.ino)?;
        self.write_cpio_word(entry.mode)?;
        self.write_cpio_word(entry.uid)?;
        self.write_cpio_word(entry.gid)?;
        self.write_cpio_word(entry.nlink)?;
        self.write_cpio_word(entry.mtime)?;
        self.write_cpio_word(entry.file_size)?;
        self.write_cpio_word(entry.dev_major)?;
        self.write_cpio_word(entry.dev_minor)?;
        self.write_cpio_word(entry.rdev_major)?;
        self.write_cpio_word(entry.rdev_minor)?;
        self.write_cpio_word(entry.name.len() + 1)?;
        self.write_cpio_word(0u32)?; // CRC
        self.write(entry.name)?;
        header_size += entry.name();
        self.write(0u8)?; // Write \0 for the string.
        // Pad to a multiple of 4 bytes
        if let Some(pad) = compute_pad4(STATIC_HEADER_LEN + name.len()) {
            self.write_all(pad)?;
            header_size += pad.len();
        }
        Ok(header_size)
    }

    fn write_cpio_contents(&mut self, header_size: usize, contents: &[u8]) -> Result<usize, acid_io::Error> {
        let mut total_size = header_size + contents.len();
        self.write_all(contents)?;
        if let Some(pad) = compute_pad4(total_size) {
            self.write_all(pad)?;
            total_size += pad.len();
        }
        Ok(total_size)
    }

    fn write_cpio_entry(&mut self, header: Entry, contents: &[u8]) -> Result<usize, acid_io::Error> {
        let header_size = self.write_cpio_header(entry)?;

        self.write_cpio_contents(header_size, contents)
    }
}

impl <W: Write + ?Sized> WriteBytesExt for W {}

// A Cpio archive with convenience methods
// to pack stuff into it.
struct Cpio {
    buffer: Vec<u8>,
    inode_counter: u32
}

impl Cpio {
    fn pack_one(&mut self, fname: &CStr16, contents: &[u8], target_dir_prefix: &str, access_mode: u32) -> uefi::Result
        {
            // cpio cannot deal with > 32 bits file sizes
            // SAFETY: u32::MAX as usize can wrap if usize < u32.
            // hopefully, I will never encounter a usize = u16 in the wild.
            if contents.len() > (u32::MAX as usize) {
                return Err(uefi::Status::LOAD_ERROR.into());
            }

            // cpio cannot deal with > 2^32 - 1 inodes neither
            if self.inode_counter == u32::MAX {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            // replace by mem::size_of
            let mut current_len = STATIC_HEADER_LEN + 1; // 1 for the `/` separator

            if current_len > usize::MAX - target_dir_prefix.len() {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            current_len += target_dir_prefix.len();

            if current_len > usize::MAX - fname.num_bytes() {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            current_len += fname.num_bytes();

            // SAFETY: u32::MAX as usize can wrap if usize < u32.
            if target_dir_prefix.len() + fname.num_bytes() >= (u32::MAX as usize) {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            // Perform 4-byte alignment of current_len

            if current_len == usize::MAX {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            // Perform 4-byte alignment of contents.len()
            let aligned_contents_len = contents.len();
            if aligned_contents_len == usize::MAX {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            if current_len > usize::MAX - aligned_contents_len {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            current_len += aligned_contents_len;

            if self.buffer.len() > usize::MAX - current_len {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            // Perform re-allocation now.
            let mut elt_buffer: Vec<u8> = Vec::with_capacity(current_len);
            let cur = Cursor::new(&mut elt_buffer);

            self.inode_counter += 1;
            // TODO: perform the concat properly
            // transform fname to string
            cur.write_cpio_entry(Entry {
                name: target_dir_prefix + "/" + fname,
                ino: self.inode_counter,
                mode: access_mode | 0100000, // S_IFREG
                uid: 0,
                gid: 0,
                nlink: 1,
                mtime: 0,
                file_size: contents.len(),
                dev_major: 0,
                dev_minor: 0,
                rdev_major: 0,
                rdev_minor: 0
            }, contents)?;

            // Concat the element buffer.
            self.buffer.append(&mut element_buffer);

            Ok(())
        }
    fn pack_dir(&mut self, path: &str, access_mode: u32) -> uefi::Result {
        // cpio cannot deal with > 2^32 - 1 inodes neither
        if self.inode_counter == u32::MAX {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        let current_len = STATIC_HEADER_LEN;
        if current_len > usize::MAX - path.len() {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        current_len += path.len();

        // Align the whole header
        if self.buffer.len() == usize::MAX || self.buffer.len() > usize::MAX - current_len {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        let mut elt_buffer: Vec<u8> = Vec::with_capacity(current_len);
        let cur = Cursor::new(&mut elt_buffer);

        self.inode_counter += 1;
        cur.write_cpio_header(Entry {
            name: path.into(),
            ino: self.inode_counter,
            mode: access_mode | 0100000, // S_IFREG
            uid: 0,
            gid: 0,
            nlink: 1,
            mtime: 0,
            file_size: 0,
            dev_major: 0,
            dev_minor: 0,
            rdev_major: 0,
            rdev_minor: 0
        })?;

        // Concat the element buffer.
        self.buffer.append(&mut element_buffer);

        Ok(())
    }

    fn pack_prefix(&mut self, path: &str, dir_mode: u32) -> uefi::Result {
        // Iterate over all parts of `path`
        // pack_dir it
        Ok(())
    }

    fn pack_trailer(&mut self) -> uefi::Result {
        self.pack_one("", TRAILER_NAME, "", 0x0)
    }
}


fn pack_cpio(
    boot_services: &BootServices,
    fs: SimpleFileSystem,
    dropin_dir: Option<&CStr16>,
    match_suffix: &CStr16,
    target_dir_prefix: &str,
    dir_mode: u32,
    access_mode: u32,
    tpm_pcr: PcrIndex,
    tpm_description: &str) -> uefi::Result<Option<Cpio>> {
    match fs.open_volume() {
        Some(root_dir) => {
            let real_dropin_dir: CString16 = dropin_dir.or_else(get_dropin_dir);
            // open_directory???
        },
        Err(uefi::Status::UNSUPPORTED) => Ok(None),
        // Log the error.
        err => err
    }
}

fn pack_cpio_literal(
    boot_services: &BootServices,
    data: &Vec<u8>,
    target_dir_prefix: &str,
    target_filename: &CStr16,
    dir_mode: u32,
    access_mode: u32,
    tpm_pcr: PcrIndex,
    tpm_description: &str) -> uefi::Result<Cpio> {
    let cpio = Cpio {
        buffer: Vec::new(),
        inode_counter: 0
    };

    cpio.pack_prefix(target_dir_prefix, dir_mode)?;
    cpio.pack_one(
        target_filename,
        data,
        target_dir_prefix,
        access_mode)?;
    cpio.pack_trailer()?;
    tpm_log_event_ascii(boot_services, pcr_index, data, tpm_description)?;

    Ok(cpio)
}
