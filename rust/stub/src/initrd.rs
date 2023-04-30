use crate::{cpio::Cpio, uefi_helpers::SD_LOADER, measure::TPM_PCR_INDEX_KERNEL_PARAMETERS};
use alloc::vec::Vec;
use uefi::{prelude::RuntimeServices, table::runtime::VariableAttributes, cstr16};

pub enum CompanionInitrd {
    Credentials(Cpio),
    GlobalCredentials(Cpio),
    SystemExtension(Cpio),
    PcrSignature(Cpio),
    PcrPublicKey(Cpio)
}

pub fn export_pcr_efi_variables(runtime_services: &RuntimeServices,
    initrds: &Vec<CompanionInitrd>) -> uefi::Result {
    // Do we have kernel parameters that were measured
    if initrds.iter().any(|e| match e {
        CompanionInitrd::Credentials(_) => true,
        CompanionInitrd::GlobalCredentials(_) => true,
        _ => false
    }) {
        runtime_services.set_variable(
            cstr16!("StubPcrKernelParameters"),
            &SD_LOADER,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &TPM_PCR_INDEX_KERNEL_PARAMETERS.0.to_le_bytes()
        )?;
    }
    // Do we have system extensions that were measured
    if initrds.iter().any(|e| match e {
        CompanionInitrd::SystemExtension(_) => true,
        _ => false
    }) {
        runtime_services.set_variable(
            cstr16!("StubPcrInitRDSysExts"),
            &SD_LOADER,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &TPM_PCR_INDEX_KERNEL_PARAMETERS.0.to_le_bytes()
        )?;
    }

    Ok(())
}
