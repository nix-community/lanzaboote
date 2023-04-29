use alloc::vec;
use core::mem::{self, MaybeUninit};
use log::warn;
use uefi::{
    prelude::BootServices,
    proto::tcg::{
        v1::{self, Sha1Digest},
        v2, EventType, PcrIndex,
    },
    table::boot::ScopedProtocol,
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

fn open_capable_tpm1(boot_services: &BootServices) -> uefi::Result<ScopedProtocol<v1::Tcg>> {
    let tpm_handle = boot_services.get_handle_for_protocol::<v1::Tcg>()?;
    let mut tpm_protocol = boot_services.open_protocol_exclusive::<v1::Tcg>(tpm_handle)?;

    let status_check = tpm_protocol.status_check()?;

    if status_check.protocol_capability.tpm_deactivated()
        || !status_check.protocol_capability.tpm_present()
    {
        warn!("Capability `TPM present` is not there or `TPM deactivated` is there for the existing TPM TCGv1 protocol");
        return Err(uefi::Status::UNSUPPORTED.into());
    }

    Ok(tpm_protocol)
}

pub fn tpm_available(boot_services: &BootServices) -> bool {
    open_capable_tpm2(boot_services).is_ok() || open_capable_tpm1(boot_services).is_ok()
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
        let required_size = mem::size_of::<u32>()
            // EventHeader is private…
            + mem::size_of::<u32>() + mem::size_of::<u16>() + mem::size_of::<PcrIndex>() + mem::size_of::<EventType>()
            + description.len();

        let mut event_buffer = vec![MaybeUninit::<u8>::uninit(); required_size];
        let event = v2::PcrEventInputs::new_in_buffer(
            event_buffer.as_mut_slice(),
            pcr_index,
            EventType::IPL,
            description.as_bytes(),
        )?;
        // FIXME: what do we want as flags here?
        tpm2.hash_log_extend_event(Default::default(), buffer, event)?;
    } else if let Ok(mut tpm1) = open_capable_tpm1(boot_services) {
        let required_size = mem::size_of::<PcrIndex>()
            + mem::size_of::<EventType>()
            + mem::size_of::<Sha1Digest>()
            + mem::size_of::<u32>()
            + description.len();

        let mut event_buffer = vec![MaybeUninit::<u8>::uninit(); required_size];

        // Compute sha1 of the event data
        let mut m = sha1_smol::Sha1::new();
        m.update(description.as_bytes());

        let event = v1::PcrEvent::new_in_buffer(
            event_buffer.as_mut_slice(),
            pcr_index,
            EventType::IPL,
            m.digest().bytes(),
            description.as_bytes(),
        )?;

        tpm1.hash_log_extend_event(event, Some(buffer))?;
    }

    Ok(true)
}
