use uefi::{CStr16, cstr16};
use alloc::{vec, vec::Vec, string::String, format};
use embedded_io::{Write, ErrorType, ErrorKind, Error};

const MAGIC_NUMBER: &[u8; 6] = b"070701";
const TRAILER_NAME: &str= "TRAILER!!!";
const CPIO_HEX: &[u8; 16] = b"0123456789abcdef";

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
    // unstable for const fn yet: https://github.com/rust-lang/rust/issues/46571
    // + core::mem::size_of_val(MAGIC_NUMBER)
    + core::mem::size_of::<&[u8; 6]>() // = 6
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

trait WriteBytesExt : Write {
    fn write_cpio_word(&mut self, word: u32) -> Result<(), Self::Error> {
        // A CPIO word is the hex(word) written as chars.
        // We do it manually because format! will allocate.
        self.write_all(
            &word.to_le_bytes()
            .into_iter()
            .enumerate()
            // u8 -> usize is always safe.
            .map(|(i, c)| CPIO_HEX[((c >> (4 * i)) & 0xF) as usize])
            .rev()
            .collect::<Vec<u8>>()
        )
    }

    fn write_cpio_header(&mut self, entry: Entry) -> Result<usize, Self::Error> {
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
        self.write_cpio_word((entry.name.len() + 1).try_into().expect("Filename cannot be longer than a 32-bits size"))?;
        self.write_cpio_word(0u32)?; // CRC
        self.write_all(entry.name.as_bytes())?;
        header_size += entry.name.len();
        self.write(&[0u8])?; // Write \0 for the string.
        // Pad to a multiple of 4 bytes
        if let Some(pad) = compute_pad4(STATIC_HEADER_LEN + entry.name.len()) {
            self.write_all(&pad)?;
            header_size += pad.len();
        }
        Ok(header_size)
    }

    fn write_cpio_contents(&mut self, header_size: usize, contents: &[u8]) -> Result<usize, Self::Error> {
        let mut total_size = header_size + contents.len();
        self.write_all(contents)?;
        if let Some(pad) = compute_pad4(total_size) {
            self.write_all(&pad)?;
            total_size += pad.len();
        }
        Ok(total_size)
    }

    fn write_cpio_entry(&mut self, header: Entry, contents: &[u8]) -> Result<usize, Self::Error> {
        let header_size = self.write_cpio_header(header)?;

        self.write_cpio_contents(header_size, contents)
    }
}

impl <W: Write + ?Sized> WriteBytesExt for W {}

struct MemoryCursor<'a> {
    buffer: &'a mut Vec<u8>
}

impl<'a> MemoryCursor<'a> {
    fn new(buffer: &'a mut Vec<u8>) -> Self {
        Self {
            buffer
        }
    }
}

#[derive(Debug)]
struct UefiError(uefi::Error);

impl Error for UefiError {
    fn kind(&self) -> ErrorKind {
        match self.0.status() {
            uefi::Status::UNSUPPORTED => ErrorKind::Unsupported,
            uefi::Status::IP_ADDRESS_CONFLICT => ErrorKind::AddrInUse,
            uefi::Status::INVALID_PARAMETER => ErrorKind::InvalidInput,
            uefi::Status::TIMEOUT => ErrorKind::TimedOut,
            uefi::Status::NOT_READY => ErrorKind::Interrupted,
            uefi::Status::OUT_OF_RESOURCES => ErrorKind::OutOfMemory,
            _ => ErrorKind::Other
        }
    }
}

impl ErrorType for MemoryCursor<'_> {
    type Error = UefiError;
}

impl Write for MemoryCursor<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> { 
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A CPIO archive with convenience methods
/// to pack a file hierarchy inside.
pub struct Cpio {
    buffer: Vec<u8>,
    inode_counter: u32
}

impl From<Cpio> for Vec<u8> {
    fn from(value: Cpio) -> Self {
        value.buffer
    }
}

impl Cpio {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            inode_counter: 0
        }
    }

    pub fn pack_one(&mut self, fname: &CStr16, contents: &[u8], target_dir_prefix: &str, access_mode: u32) -> uefi::Result
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
            current_len = align::<4>(current_len);
            if current_len == usize::MAX {
                return Err(uefi::Status::OUT_OF_RESOURCES.into());
            }

            // Perform 4-byte alignment of contents.len()
            let aligned_contents_len = align::<4>(contents.len());
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
            let mut cur = MemoryCursor::new(&mut elt_buffer);

            self.inode_counter += 1;
            // TODO: perform the concat properly
            // transform fname to string
            cur.write_cpio_entry(Entry {
                name: format!("{}/{}", target_dir_prefix, fname),
                ino: self.inode_counter,
                mode: access_mode | 0100000, // S_IFREG
                uid: 0,
                gid: 0,
                nlink: 1,
                mtime: 0,
                // This was checked previously.
                file_size: contents.len().try_into().unwrap(),
                dev_major: 0,
                dev_minor: 0,
                rdev_major: 0,
                rdev_minor: 0
            }, contents).map_err(|_err| uefi::Status::BAD_BUFFER_SIZE)?;

            // Concat the element buffer.
            self.buffer.append(&mut elt_buffer);

            Ok(())
        }
    pub fn pack_dir(&mut self, path: &str, access_mode: u32) -> uefi::Result {
        // cpio cannot deal with > 2^32 - 1 inodes neither
        if self.inode_counter == u32::MAX {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        let mut current_len = STATIC_HEADER_LEN;
        if current_len > usize::MAX - path.len() {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        current_len += path.len();

        // Align the whole header
        current_len = align::<4>(current_len);
        if self.buffer.len() == usize::MAX || self.buffer.len() > usize::MAX - current_len {
            return Err(uefi::Status::OUT_OF_RESOURCES.into());
        }

        let mut elt_buffer: Vec<u8> = Vec::with_capacity(current_len);
        let mut cur = MemoryCursor::new(&mut elt_buffer);

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
        }).map_err(|_err| uefi::Status::BAD_BUFFER_SIZE)?;

        // Concat the element buffer.
        self.buffer.append(&mut elt_buffer);

        Ok(())
    }

    pub fn pack_prefix(&mut self, path: &str, dir_mode: u32) -> uefi::Result {
        // TODO: bring Unix paths inside UEFI
        // and just reuse &Path there and iterate over ancestors().rev()?
        let mut ancestor = String::new();
        for component in path.split('/') {
            ancestor = ancestor + "/" + component;
            self.pack_dir(&ancestor, 0o555)?;
        }
        Ok(())
    }

    pub fn pack_trailer(&mut self) -> uefi::Result {
        self.pack_one(cstr16!("."), TRAILER_NAME.as_bytes(), "", 0)
    }
}


