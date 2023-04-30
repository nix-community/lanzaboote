use uefi::{table::{runtime::VariableAttributes, Boot, SystemTable}, cstr16, proto::tcg::PcrIndex};
use alloc::vec::Vec;
use crate::{uefi_helpers::{PeInMemory, SD_LOADER}, pe_section::pe_section_data, unified_sections::UnifiedSection, tpm::tpm_log_event_ascii};

/// This is the TPM PCR where lanzastub extends its payload into, before using them.
/// All unified sections: kernel ELF image, embedded initrd, etc.
/// Contrary to PCR4, it contains precomputable data because PCR4 contains
/// the whole PE measured, this PCR is made of "static data".
pub const TPM_PCR_INDEX_KERNEL_IMAGE: PcrIndex = PcrIndex(11);
/// This is the TPM PCR where lanzastub extends the kernel command line and any passed credentials
/// into.
pub const TPM_PCR_INDEX_KERNEL_PARAMETERS: PcrIndex = PcrIndex(12);
/// This is the TPM PCR where lanzastub extends the initrd system extension images into
/// which we pass to the booted kernel.
pub const TPM_PCR_INDEX_INITRD_SYSEXTS: PcrIndex = PcrIndex(13);
/// This is the TPM PCR where we measure the root fs volume key (and maybe /var/'s) if it is split
/// off.
/// Unused at the moment in lanzastub.
pub const TPM_PCR_INDEX_VOLUME_KEY: PcrIndex = PcrIndex(15);

pub unsafe fn measure_image(
    system_table: &SystemTable<Boot>,
    image: PeInMemory) -> uefi::Result<(u32, Vec<UnifiedSection>)> {
    let runtime_services = system_table.runtime_services();
    let boot_services = system_table.boot_services();

    let pe_binary = unsafe { image.as_slice() };
    let pe = goblin::pe::PE::parse(pe_binary)
        .map_err(|_err| uefi::Status::LOAD_ERROR)?;

    let mut measurements = 0;
    // We would like to use core::mem::variant_count
    // https://github.com/rust-lang/rust/issues/73662
    let mut unified_sections = Vec::with_capacity(8);
    for section in pe.sections {
        let section_name = section.name().map_err(|_err| uefi::Status::UNSUPPORTED)?;
        if let Ok(unified_section) = UnifiedSection::from_section_table(&pe_binary, &section) {
            // UNSTABLE: && in the previous if is an unstable feature
            // https://github.com/rust-lang/rust/issues/53667
            if unified_section.should_be_measured() {
                // Here, perform the TPM log event in ASCII.
                if let Some(data) = pe_section_data(pe_binary, &section) {
                    if tpm_log_event_ascii(boot_services, TPM_PCR_INDEX_KERNEL_IMAGE, data, section_name)? {
                        measurements += 1;
                    }
                }
            }
            // We would like to use push_within_capacity and err out correctly
            // if the capacity is exceeded.
            // https://github.com/rust-lang/rust/issues/100486
            unified_sections.push(unified_section);
        }
    }

    if measurements > 0 {
        // If we did some measurements, expose a variable encoding the PCR where
        // we have done the measurements.
        runtime_services.set_variable(
            cstr16!("StubPcrKernelImage"),
            &SD_LOADER,
            VariableAttributes::from_bits_truncate(0x0),
            &TPM_PCR_INDEX_KERNEL_IMAGE.0.to_le_bytes()
        )?;
    }

    Ok((measurements, unified_sections))
}
