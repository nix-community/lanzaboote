use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tempfile::TempDir;

/// Extension for a temporary directory that enables creating secure temporary files in it.
pub trait SecureTempDirExt {
    fn create_secure_file(&self, file_name: &str) -> Result<fs::File>;
    fn write_secure_file(&self, file_name: &str, contents: impl AsRef<[u8]>) -> Result<PathBuf>;
}

/// This implementation has three useful properties:
///
/// - Files are created with mode 0o600, so that they are only accessible by the current user.
/// - Files are named and not ephemeral (unlike a real temporary file).
/// - The directory and its children are cleaned up (i.e. deleted) when the variable that holds the
///   directory goes out of scope.
///
/// This protects against an attacker _without_ root access from modifying files undetected. It
/// provides no prection against an attacker _with_ root access. Additionally, because the files
/// have named paths, they can be passed to external programs while still being securely deleted
/// after they are not needed anymore.
impl SecureTempDirExt for TempDir {
    /// Create a temporary file that can only be accessed by the current Linux user.
    fn create_secure_file(&self, file_name: &str) -> Result<fs::File> {
        let path = self.path().join(file_name);
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("Failed to create tempfile: {path:?}"))
    }

    /// Create a temporary file and write a `u8` slice to it.
    fn write_secure_file(&self, file_name: &str, contents: impl AsRef<[u8]>) -> Result<PathBuf> {
        let path = self.path().join(file_name);
        let mut tmpfile = self.create_secure_file(file_name)?;

        tmpfile
            .write_all(contents.as_ref())
            .with_context(|| format!("Failed to write to tempfile {path:?}"))?;

        Ok(path)
    }
}
