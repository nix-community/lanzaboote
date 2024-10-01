use alloc::vec::Vec;
use log::warn;
use uefi::{
    prelude::BootServices,
    proto::tcg::{v2, EventType, PcrIndex},
    table::boot::ScopedProtocol,
    ResultExt,
};

fn open_capable_tpm2(boot_services: &BootServices) -> uefi::Result<ScopedProtocol<v2::Tcg>> {
    let tpm_handle = boot_services.get_handle_for_protocol::<v2::Tcg>()?;
    let mut tpm_protocol = boot_services.open_protocol_exclusive::<v2::Tcg>(tpm_handle)?;

    let capabilities = tpm_protocol.get_capability()?;

    /*
     * Here's systemd-stub perform a cast to EFI_TCG_BOOT_SERVICE_CAPABILITY
     * indicating there could be some quirks to workaround.
     * It should probably go to uefi-rs?
    if capabilities.structure_version.major == 1 && capabilities.structure_version.minor == 0 {

    }*/

    if !capabilities.tpm_present() {
        warn!("Capability `TPM present` is not there for the existing TPM TCGv2 protocol");
        return Err(uefi::Status::UNSUPPORTED.into());
    }

    Ok(tpm_protocol)
}

pub fn tpm_available(boot_services: &BootServices) -> bool {
    open_capable_tpm2(boot_services).is_ok()
}

/// Log an event in the TPM with `buffer` as data.
/// Returns a boolean whether the measurement has been done or not in case of success.
pub fn tpm_log_event_ascii(
    boot_services: &BootServices,
    pcr_index: PcrIndex,
    buffer: &[u8],
    description: &str,
) -> uefi::Result<bool> {
    if pcr_index.0 == u32::MAX {
        return Ok(false);
    }
    if let Ok(mut tpm2) = open_capable_tpm2(boot_services) {
        let description_encoded = description
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<_>>();

        let event = v2::PcrEventInputs::new_in_box(pcr_index, EventType::IPL, &description_encoded)
            .discard_errdata()?;
        // FIXME: what do we want as flags here?
        tpm2.hash_log_extend_event(Default::default(), buffer, &event)?;
    }

    Ok(true)
}
