use snafu::prelude::Snafu;

#[derive(Debug, Snafu)]
pub enum CPIOError<IOError: embedded_io::Error + core::fmt::Debug> {
    #[snafu(display("File size does not fit in 32 bits ({got})"))]
    TooLargeFileSize { got: usize },
    #[snafu(display("This CPIO archive is exceeding the maximum amount of inodes (2^32 - 1)"))]
    MaximumInodesReached,
    #[snafu(display(
        "This CPIO archive is too large to fit inside of a 64 bits integer in terms of buffer size"
    ))]
    MaximumArchiveReached,
    #[snafu(display(
        "Provided buffer size is too small, expected: {expected} bytes, got: {got} bytes"
    ))]
    InsufficientBufferSize { expected: usize, got: usize },
    #[snafu(display("An IO error was encountered: {src:?}"))]
    IOError { src: IOError },
}
