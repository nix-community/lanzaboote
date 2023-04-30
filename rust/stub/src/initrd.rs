use crate::cpio::Cpio;
use alloc::vec::Vec;

pub enum CompanionInitrd {
    Credentials(Cpio),
    GlobalCredentials(Cpio),
    SystemExtension(Cpio),
    PcrSignature(Cpio),
    PcrPublicKey(Cpio)
}

pub fn export_pcr_efi_variables(initrds: Vec<CompanionInitrd>) -> uefi::Result {
    Ok(())
}
