use alloc::vec::Vec;
use uefi::{proto::media::file::RegularFile, Result};

// Read the whole file into a vector.
pub fn read_all(file: &mut RegularFile) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    loop {
        let mut chunk = [0; 512];
        let read_bytes = file.read(&mut chunk).map_err(|e| e.status())?;

        if read_bytes == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[0..read_bytes]);
    }

    Ok(buf)
}
