use alloc::{string::ToString, vec::Vec};
use log::info;
use uefi::{
    cstr16,
    proto::tcg::PcrIndex,
    runtime::{self, VariableAttributes},
};

use crate::{
    companions::{CompanionInitrd, CompanionInitrdType},
    efivars::BOOT_LOADER_VENDOR_UUID,
    pe_section::pe_section_data,
    tpm::tpm_log_event_ascii,
    uefi_helpers::PeInMemory,
    unified_sections::UnifiedSection,
};

/// This is where any stub payloads are extended, e.g. kernel ELF image, embedded initrd
/// and so on.
/// Compared to PCR4, this contains only the unified sections rather than the whole PE image as-is.
///
/// Per the [UKI specification](https://uapi-group.org/specifications/specs/unified_kernel_image/#uki-tpm-pcr-measurements):
/// "For each section two measurements shall be made into PCR 11 with the event code EV_IPL:
///
/// 1. The section name in ASCII (including one trailing NUL byte)
/// 2. The (binary) section contents"
///
/// Measurements are made in canonical order, interleaved: section name, section data, next section name, etc.
const TPM_PCR_INDEX_KERNEL_IMAGE: PcrIndex = PcrIndex(11);
/// This is where lanzastub extends the kernel command line and any passed credentials into
const TPM_PCR_INDEX_KERNEL_CONFIG: PcrIndex = PcrIndex(12);
/// This is where we extend the initrd sysext images into which we pass to the booted kernel
const TPM_PCR_INDEX_SYSEXTS: PcrIndex = PcrIndex(13);

pub fn measure_image(image: &PeInMemory) -> uefi::Result<u32> {
    // SAFETY: We get a slice that represents our currently running
    // image and then parse the PE data structures from it. This is
    // safe, because we don't touch any data in the data sections that
    // might conceivably change while we look at the slice.
    // (data sections := all unified sections that can be measured.)
    let pe_binary = unsafe { image.as_slice() };
    let pe = goblin::pe::PE::parse(pe_binary).map_err(|_err| uefi::Status::LOAD_ERROR)?;

    // Build a list of (unified_section, pe_section) pairs and sort by canonical order.
    // Per UKI spec: "shall measure the sections listed above, starting from the .linux section,
    // in the order as listed (which should be considered the canonical order)."
    let mut sections_to_measure = Vec::new();
    for section in &pe.sections {
        let section_name = section.name().map_err(|_err| uefi::Status::UNSUPPORTED)?;
        if let Ok(unified_section) = UnifiedSection::try_from(section_name) {
            if unified_section.should_be_measured() {
                sections_to_measure.push((unified_section, section));
            }
        }
    }
    sections_to_measure.sort_by_key(|(unified_section, _)| *unified_section);

    let mut measurements = 0;
    for (unified_section, section) in sections_to_measure {
        if let Some(data) = pe_section_data(pe_binary, section) {
            let section_name = unified_section.name();
            info!("Measuring section `{}`...", section_name);

            // Per UKI spec: "For each section two measurements shall be made into PCR 11"
            // 1. "The section name in ASCII (including one trailing NUL byte)"
            let section_name_ascii = alloc::format!("{}\0", section_name);
            if tpm_log_event_ascii(
                TPM_PCR_INDEX_KERNEL_IMAGE,
                section_name_ascii.as_bytes(),
                section_name,
            )? {
                measurements += 1;
            }

            // 2. "The (binary) section contents"
            if tpm_log_event_ascii(TPM_PCR_INDEX_KERNEL_IMAGE, &data, section_name)? {
                measurements += 1;
            }
        }
    }

    if measurements > 0 {
        let pcr_index_encoded = TPM_PCR_INDEX_KERNEL_IMAGE
            .0
            .to_string()
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>();

        // If we did some measurements, expose a variable encoding the PCR where
        // we have done the measurements.
        runtime::set_variable(
            cstr16!("StubPcrKernelImage"),
            &BOOT_LOADER_VENDOR_UUID,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &pcr_index_encoded,
        )?;
    }

    Ok(measurements)
}

/// Performs all the expected measurements for any list of
/// companion initrds of any form.
///
/// Relies on the passed order of `companions` for measurements in the same PCR.
/// A stable order is expected for measurement stability.
pub fn measure_companion_initrds(companions: &[CompanionInitrd]) -> uefi::Result<u32> {
    let mut measurements = 0;
    let mut credentials_measured = 0;
    let mut sysext_measured = false;

    for initrd in companions {
        match initrd.r#type {
            CompanionInitrdType::PcrSignature | CompanionInitrdType::PcrPublicKey => {
                continue;
            }
            CompanionInitrdType::Credentials => {
                if tpm_log_event_ascii(
                    TPM_PCR_INDEX_KERNEL_CONFIG,
                    initrd.cpio.as_ref(),
                    "Credentials initrd",
                )? {
                    measurements += 1;
                    credentials_measured += 1;
                }
            }
            CompanionInitrdType::GlobalCredentials => {
                if tpm_log_event_ascii(
                    TPM_PCR_INDEX_KERNEL_CONFIG,
                    initrd.cpio.as_ref(),
                    "Global credentials initrd",
                )? {
                    measurements += 1;
                    credentials_measured += 1;
                }
            }
            CompanionInitrdType::SystemExtension => {
                if tpm_log_event_ascii(
                    TPM_PCR_INDEX_SYSEXTS,
                    initrd.cpio.as_ref(),
                    "System extension initrd",
                )? {
                    measurements += 1;
                    sysext_measured = true;
                }
            }
        }
    }

    if credentials_measured > 0 {
        runtime::set_variable(
            cstr16!("StubPcrKernelParameters"),
            &BOOT_LOADER_VENDOR_UUID,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &TPM_PCR_INDEX_KERNEL_CONFIG.0.to_le_bytes(),
        )?;
    }

    if sysext_measured {
        runtime::set_variable(
            cstr16!("StubPcrInitRDSysExts"),
            &BOOT_LOADER_VENDOR_UUID,
            VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS,
            &TPM_PCR_INDEX_SYSEXTS.0.to_le_bytes(),
        )?;
    }

    Ok(measurements)
}
