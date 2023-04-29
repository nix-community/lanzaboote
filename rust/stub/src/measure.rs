use uefi::{prelude::RuntimeServices, table::runtime::VariableAttributes, cstr16};

use crate::{uefi_helpers::{PeInMemory, SD_LOADER}, pe_section::pe_section_data, unified_sections::UnifiedSection};

const TPM_PCR_INDEX_KERNEL_IMAGE: u8 = 11;

pub unsafe fn measure_image(runtime_services: &RuntimeServices,
    image: PeInMemory) -> uefi::Result<u32> {
    let pe_binary = unsafe { image.as_slice() };
    let pe = goblin::pe::PE::parse(pe_binary)
        .map_err(|err| uefi::Status::LOAD_ERROR)?;

    let mut measurements = 0;
    for section in pe.sections {
        let section_name = section.name().map_err(|err| uefi::Status::UNSUPPORTED)?;
        if let Ok(unified_section) = UnifiedSection::try_from(section_name) {
            // UNSTABLE: && in the previous if is an unstable feature
            // https://github.com/rust-lang/rust/issues/53667
            if unified_section.should_be_measured() {
                let data = pe_section_data(pe_binary, &section);
                // Here, perform the TPM log event in ASCII.
                // Check if we measured anything.
                // Increment `measurements` if so.
            }
        }
    }

    if measurements > 0 {
        // If we did some measurements, expose a variable encoding the PCR where
        // we have done the measurements.
        runtime_services.set_variable(
            cstr16!("StubPcrKernelImage"),
            &SD_LOADER,
            VariableAttributes::from_bits_truncate(0x0),
            &TPM_PCR_INDEX_KERNEL_IMAGE.to_le_bytes()
        );
    }

    Ok(measurements)
}
