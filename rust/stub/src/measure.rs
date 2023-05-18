use log::info;
use uefi::{
    cstr16,
    proto::tcg::PcrIndex,
    table::{runtime::VariableAttributes, Boot, SystemTable},
};

use crate::{
    efivars::BOOT_LOADER_VENDOR_UUID, pe_section::pe_section_data, tpm::tpm_log_event_ascii,
    uefi_helpers::PeInMemory, unified_sections::UnifiedSection,
};

const TPM_PCR_INDEX_KERNEL_IMAGE: PcrIndex = PcrIndex(11);

pub unsafe fn measure_image(
    system_table: &SystemTable<Boot>,
    image: PeInMemory,
) -> uefi::Result<u32> {
    let runtime_services = system_table.runtime_services();
    let boot_services = system_table.boot_services();

    // SAFETY: We get a slice that represents our currently running
    // image and then parse the PE data structures from it. This is
    // safe, because we don't touch any data in the data sections that
    // might conceivably change while we look at the slice.
    // (data sections := all unified sections that can be measured.)
    let pe_binary = unsafe { image.as_slice() };
    let pe = goblin::pe::PE::parse(pe_binary).map_err(|_err| uefi::Status::LOAD_ERROR)?;

    let mut measurements = 0;
    for section in pe.sections {
        let section_name = section.name().map_err(|_err| uefi::Status::UNSUPPORTED)?;
        if let Ok(unified_section) = UnifiedSection::try_from(section_name) {
            // UNSTABLE: && in the previous if is an unstable feature
            // https://github.com/rust-lang/rust/issues/53667
            if unified_section.should_be_measured() {
                // Here, perform the TPM log event in ASCII.
                if let Some(data) = pe_section_data(pe_binary, &section) {
                    info!("Measuring section `{}`...", section_name);
                    if tpm_log_event_ascii(
                        boot_services,
                        TPM_PCR_INDEX_KERNEL_IMAGE,
                        data,
                        section_name,
                    )? {
                        measurements += 1;
                    }
                }
            }
        }
    }

    if measurements > 0 {
        // If we did some measurements, expose a variable encoding the PCR where
        // we have done the measurements.
        runtime_services.set_variable(
            cstr16!("StubPcrKernelImage"),
            &BOOT_LOADER_VENDOR_UUID,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &TPM_PCR_INDEX_KERNEL_IMAGE.0.to_le_bytes(),
        )?;
    }

    Ok(measurements)
}
