use anyhow::Result;
use std::path::Path;

use crate::pe::StubParameters;

pub trait LanzabooteSigner {
    /// Tries to sign a Nix store path at this location.
    /// The implementation can fail if the provided path is not a Nix store path,
    /// or, is not a trusted Nix store path, or is not a PE binary.
    /// Once the store path is signed, you are expected to return the signed contents.
    fn sign_store_path(&self, store_path: &Path) -> Result<Vec<u8>>;

    /// Assembles a stub using the tool of your choice, you can use
    /// [`crate::pe::lanzaboote_image`] for this.
    /// Once the stub is assembled, you are expected to sign it and returns its binary
    /// representation.
    fn build_and_sign_stub(&self, stub: &StubParameters) -> Result<Vec<u8>>;

    /// Returns an opaque public key, used for tools to derive content-addressability
    /// of the various files generated and installed in the ESP.
    /// This way, if the key changes, all the bootables will be different.
    fn get_public_key(&self) -> Result<Vec<u8>>;

    /// Assumes that `from` points at a PE binary and installs a signed copy of `from` at `to`.
    fn sign_and_copy(&self, from: &Path, to: &Path) -> Result<()> {
        Ok(std::fs::write(to, self.sign_store_path(from)?)?)
    }

    /// Verify the signature of a PE binary, provided as bytes.
    /// Return true if the signature was verified.
    fn verify(&self, pe_binary: &[u8]) -> Result<bool>;

    /// Verify the signature of a PE binary, provided by its path.
    /// Return true if the signature was verified.
    fn verify_path(&self, from: &Path) -> Result<bool> {
        self.verify(&std::fs::read(from).expect("Failed to read the path to verify"))
    }
}

pub mod local;
