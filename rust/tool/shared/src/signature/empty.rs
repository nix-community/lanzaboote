use crate::pe::lanzaboote_image;
use std::path::Path;

use anyhow::{Context, Result};
use tempfile::tempdir;

use super::Signer;

/// An empty key pair.
///
/// Useful for installing files to the ESP without actually signing them for example when
/// installing a system for the first time and auto generating keys on the first boot.
#[derive(Debug, Clone, Default)]
pub struct EmptyKeyPair;

impl Signer for EmptyKeyPair {
    fn get_public_key(&self) -> Result<Vec<u8>> {
        Ok(b"unsigned".to_vec())
    }

    fn sign_and_copy(&self, from: &Path, to: &Path) -> Result<()> {
        std::fs::copy(from, to).with_context(|| {
            format!(
                "Failed to copy file from {} to {}",
                from.display(),
                to.display()
            )
        })?;
        Ok(())
    }

    fn sign_store_path(&self, store_path: &Path) -> Result<Vec<u8>> {
        Ok(std::fs::read(store_path)?)
    }

    fn build_and_sign_stub(&self, stub: &crate::pe::StubParameters) -> Result<Vec<u8>> {
        let working_tree = tempdir()?;
        let lzbt_image_path =
            lanzaboote_image(&working_tree, stub).context("Failed to build a lanzaboote image")?;

        std::fs::read(&lzbt_image_path).context("Failed to read a lanzaboote image")
    }

    fn verify(&self, _pe_binary: &[u8]) -> Result<bool> {
        Ok(true)
    }

    fn verify_path(&self, _path: &Path) -> Result<bool> {
        Ok(true)
    }
}
