use core::marker::PhantomData;

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use embedded_io::Write;

use crate::{cursor::Cursor, errors::CPIOError};

const MAGIC_NUMBER: &[u8; 6] = b"070701";
const TRAILER_NAME: &str = "TRAILER!!!";

pub type Result<V, IOError> = core::result::Result<V, CPIOError<IOError>>;

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

const STATIC_HEADER_LEN: usize = 6 // c_magic[6]
    + (8 * 13); // c_ino, c_mode, c_uid, c_gid, c_nlink, c_mtime, c_filesize, c_devmajor,
                // c_devminor, c_rdevmajor, c_rdevminor, c_namesize, c_check, all of them being &[u8; 8].

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

/// Align on N-byte boundary a value.
fn align<const A: usize>(value: usize) -> usize {
    // Assert if A is a power of 2.
    // assert!(A & (A - 1) == 0);

    if value > usize::MAX - (A - 1) {
        usize::MAX
    } else {
        (value + A - 1) & !(A - 1)
    }
}

trait WriteBytesExt: Write {
    fn write_cpio_word(&mut self, word: u32) -> core::result::Result<(), Self::Error> {
        // A CPIO word is the hex(word) written as chars.
        self.write_all(format!("{:08x}", word).as_bytes())
    }

    fn write_cpio_header(&mut self, entry: Entry) -> core::result::Result<usize, Self::Error> {
        let mut header_size = STATIC_HEADER_LEN;
        self.write_all(MAGIC_NUMBER)?;
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
        self.write_cpio_word(
            (entry.name.len() + 1)
                .try_into()
                .expect("Filename cannot be longer than a 32-bits size"),
        )?;
        self.write_cpio_word(0u32)?; // CRC
        self.write_all(entry.name.as_bytes())?;
        header_size += entry.name.len();
        self.write(&[0u8])?; // Write \0 for the string.
        header_size += 1;
        // Pad to a multiple of 4 bytes
        if let Some(pad) = compute_pad4(header_size) {
            self.write_all(&pad)?;
            header_size += pad.len();
        }
        assert!(
            header_size % 4 == 0,
            "CPIO header is not aligned on a 4-bytes boundary!"
        );
        Ok(header_size)
    }

    fn write_cpio_contents(
        &mut self,
        header_size: usize,
        contents: &[u8],
    ) -> core::result::Result<usize, Self::Error> {
        let mut total_size = header_size + contents.len();
        self.write_all(contents)?;
        if let Some(pad) = compute_pad4(contents.len()) {
            self.write_all(&pad)?;
            total_size += pad.len();
        }
        assert!(
            total_size % 4 == 0,
            "CPIO file data is not aligned on a 4-bytes boundary!"
        );
        Ok(total_size)
    }

    fn write_cpio_entry(
        &mut self,
        header: Entry,
        contents: &[u8],
    ) -> core::result::Result<usize, Self::Error> {
        let header_size = self.write_cpio_header(header)?;

        self.write_cpio_contents(header_size, contents)
    }
}

impl<W: Write + ?Sized> WriteBytesExt for W {}

/// A CPIO archive with convenience methods
/// to pack a file hierarchy inside.
pub struct Cpio<IOError: embedded_io::Error + core::fmt::Debug> {
    buffer: Vec<u8>,
    inode_counter: u32,
    _error: PhantomData<IOError>,
}

impl<I: embedded_io::Error + core::fmt::Debug> From<Cpio<I>> for Vec<u8> {
    fn from(value: Cpio<I>) -> Self {
        value.into_inner()
    }
}

impl<I: embedded_io::Error + core::fmt::Debug> AsRef<[u8]> for Cpio<I> {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

impl<IOError: embedded_io::Error + core::fmt::Debug> Default for Cpio<IOError> {
    fn default() -> Self {
        Self::new()
    }
}

impl<IOError: embedded_io::Error + core::fmt::Debug> Cpio<IOError> {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            inode_counter: 0,
            _error: PhantomData,
        }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    /// Pack inside the archive a file named `fname` containing `contents` under
    /// `target_dir_prefix` hierarchy of files with access mode specified by `access_mode`.
    /// It may return IO errors or error specific to the CPIO archives.
    pub fn pack_one(
        &mut self,
        fname: &str,
        contents: &[u8],
        target_dir_prefix: &str,
        access_mode: u32,
    ) -> Result<usize, IOError> {
        // cpio cannot deal with > 32 bits file sizes
        // SAFETY: u32::MAX as usize can wrap if usize < u32.
        // hopefully, I will never encounter a usize = u16 in the wild.
        if contents.len() > (u32::MAX as usize) {
            return Err(CPIOError::TooLargeFileSize {
                got: contents.len(),
            });
        }

        // cpio cannot deal with > 2^32 - 1 inodes neither
        if self.inode_counter == u32::MAX {
            return Err(CPIOError::MaximumInodesReached);
        }

        let mut current_len = STATIC_HEADER_LEN + 1; // 1 for the `/` separator

        if current_len > usize::MAX - target_dir_prefix.len() {
            return Err(CPIOError::MaximumArchiveReached);
        }

        current_len += target_dir_prefix.len();

        if current_len > usize::MAX - fname.len() {
            return Err(CPIOError::MaximumArchiveReached);
        }

        current_len += fname.len();

        // SAFETY: u32::MAX as usize can wrap if usize < u32.
        if target_dir_prefix.len() + fname.len() >= (u32::MAX as usize) {
            return Err(CPIOError::MaximumArchiveReached);
        }

        // Perform 4-byte alignment of current_len
        current_len = align::<4>(current_len);
        if current_len == usize::MAX {
            return Err(CPIOError::MaximumArchiveReached);
        }

        // Perform 4-byte alignment of contents.len()
        let aligned_contents_len = align::<4>(contents.len());
        if aligned_contents_len == usize::MAX {
            return Err(CPIOError::MaximumArchiveReached);
        }

        if current_len > usize::MAX - aligned_contents_len {
            return Err(CPIOError::MaximumArchiveReached);
        }

        current_len += aligned_contents_len;

        if self.buffer.len() > usize::MAX - current_len {
            return Err(CPIOError::MaximumArchiveReached);
        }

        // Perform re-allocation now.
        let mut cur = Cursor::new(Vec::with_capacity(current_len));

        self.inode_counter += 1;
        // TODO: perform the concat properly
        // transform fname to string
        let written = cur
            .write_cpio_entry(
                Entry {
                    name: if !target_dir_prefix.is_empty() {
                        format!("{}/{}", target_dir_prefix, fname)
                    } else {
                        fname.to_string()
                    },
                    ino: self.inode_counter,
                    mode: access_mode | 0o100000, // S_IFREG
                    uid: 0,
                    gid: 0,
                    nlink: 1,
                    mtime: 0,
                    // This was checked previously.
                    file_size: contents.len().try_into().unwrap(),
                    dev_major: 0,
                    dev_minor: 0,
                    rdev_major: 0,
                    rdev_minor: 0,
                },
                contents,
            )
            .unwrap(); // This is infallible as long as allocation is not failible.

        // Concat the element buffer.
        self.buffer.append(cur.get_mut());

        Ok(written)
    }
    pub fn pack_dir(&mut self, path: &str, access_mode: u32) -> Result<(), IOError> {
        // cpio cannot deal with > 2^32 - 1 inodes neither
        if self.inode_counter == u32::MAX {
            return Err(CPIOError::MaximumInodesReached);
        }

        let mut current_len = STATIC_HEADER_LEN;
        if current_len > usize::MAX - path.len() {
            return Err(CPIOError::MaximumArchiveReached);
        }

        current_len += path.len();

        // Align the whole header
        current_len = align::<4>(current_len);
        if self.buffer.len() == usize::MAX || self.buffer.len() > usize::MAX - current_len {
            return Err(CPIOError::MaximumArchiveReached);
        }

        let mut cur = Cursor::new(Vec::with_capacity(current_len));

        self.inode_counter += 1;
        cur.write_cpio_header(Entry {
            name: path.into(),
            ino: self.inode_counter,
            mode: access_mode | 0o040000, // S_IFDIR
            uid: 0,
            gid: 0,
            nlink: 1,
            mtime: 0,
            file_size: 0,
            dev_major: 0,
            dev_minor: 0,
            rdev_major: 0,
            rdev_minor: 0,
        })
        .unwrap(); // This is infallible as long as allocation is not failible.

        // Concat the element buffer.
        self.buffer.append(cur.get_mut());

        Ok(())
    }

    pub fn pack_prefix(&mut self, path: &str, dir_mode: u32) -> Result<(), IOError> {
        // TODO: bring Unix paths inside this crate?
        // and just reuse &Path there and iterate over ancestors().rev()?
        let mut ancestor = String::new();

        // This will serialize all directory inodes of all prefix paths
        // until the final directory which will be serialized with the proper `dir_mode`
        let components = path.split('/');
        let parts = components.clone().count();
        if parts == 0 {
            // packing the prefix of an empty path is trivial.
            return Ok(());
        }

        let last = components.clone().last().unwrap();
        let prefixes = components.take(parts - 1);

        for component in prefixes {
            ancestor = ancestor + "/" + component;
            self.pack_dir(&ancestor, 0o555)?;
        }

        self.pack_dir(&(ancestor + "/" + last), dir_mode)
    }

    pub fn pack_trailer(&mut self) -> Result<usize, IOError> {
        self.pack_one(TRAILER_NAME, b"", "", 0)
    }
}
