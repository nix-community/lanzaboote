use core::convert::Infallible;

use alloc::vec::Vec;
use embedded_io::{ErrorType, Write};

pub struct Cursor {
    buffer: Vec<u8>,
}

impl Cursor {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self { buffer }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    pub fn get_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }
}

impl ErrorType for Cursor {
    type Error = Infallible;
}

impl Write for Cursor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
