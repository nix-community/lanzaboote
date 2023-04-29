use uefi::{prelude::BootServices, table::{runtime::VariableAttributes, boot::ScopedProtocol}, cstr16, CStr16, proto::tcg::{v1, v2::{Tcg, PcrEventInputs, HashLogExtendEventFlags}, EventType, PcrIndex}};

fn open_capable_tpm2(boot_services: &BootServices) -> uefi::Result<ScopedProtocol<Tcg>> {
    let tpm_handle = boot_services.get_handle_for_protocol::<Tcg>()?;
    let mut tpm_protocol = boot_services.open_protocol_exclusive::<Tcg>(tpm_handle)?;

    let capabilities = tpm_protocol.get_capability()?;

    /*
     * Here's systemd-stub perform a cast to EFI_TCG_BOOT_SERVICE_CAPABILITY
     * indicating there could be some quirks to workaround.
     * It should probably go to uefi-rs?
    if capabilities.structure_version.major == 1 && capabilities.structure_version.minor == 0 {

    }*/

    if !capabilities.tpm_present() {
        return Err(uefi::Status::UNSUPPORTED.into());
    }

    Ok(tpm_protocol)
}

fn open_capable_tpm1(boot_services: &BootServices) -> uefi::Result<ScopedProtocol<v1::Tcg>> {
    let tpm_handle = boot_services.get_handle_for_protocol::<v1::Tcg>()?;
    let mut tpm_protocol = boot_services.open_protocol_exclusive::<v1::Tcg>(tpm_handle)?;

    let status_check = tpm_protocol.status_check()?;

    if status_check.protocol_capability.tpm_deactivated() || !status_check.protocol_capability.tpm_present() {
        return Err(uefi::Status::UNSUPPORTED.into());
    }

    Ok(tpm_protocol)
}

fn tpm_available(boot_services: &BootServices) -> bool {
    open_capable_tpm2(boot_services).is_ok() || open_capable_tpm1(boot_services).is_ok()
}

/// Log an event in the TPM with `buffer` as data.
/// Returns a boolean whether the measurement has been done or not in case of success.
pub fn tpm_log_event_ascii(boot_services: &BootServices,
    pcr_index: PcrIndex, buffer: &[u8], description: &str) -> uefi::Result<bool> {
    if pcr_index.0 == u32::MAX {
        return Ok(false);
    }

    if let Ok(tpm2) = open_capable_tpm2(boot_services) {
        let mut event_buffer = vec![0; 100];
        let event = PcrEventInputs::new_in_buffer(&mut event_buffer, pcr_index, EventType::IPL, description.as_bytes())?;
        // FIXME: what do we want as flags here?
        tpm2.hash_log_extend_event(Default::default(), buffer, event);
    } else if let Ok(tpm1) = open_capable_tpm1(boot_services) {
        let mut event_buffer = vec![0; 100];
        let digest;
        // FIXME: sha1
        let event = v1::PcrEvent::new_in_buffer(&mut event_buffer, pcr_index,
            EventType::IPL,
            digest,
            description.as_bytes())?;

        tpm1.hash_log_extend_event(event, Some(buffer))?;
    }

    Ok(true)
}

