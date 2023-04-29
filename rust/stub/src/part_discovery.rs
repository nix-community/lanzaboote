use uefi::{Result, Handle, prelude::BootServices, proto::device_path::{DevicePath, media::{HardDrive, PartitionSignature}, DeviceType, DeviceSubType}, Guid};

pub fn disk_get_part_uuid(boot_services: &BootServices, disk_handle: Handle) -> Result<Guid> {
    let dp = boot_services.open_protocol_exclusive::<DevicePath>(disk_handle)?;

    for node in dp.node_iter() {
        if node.device_type() != DeviceType::MEDIA || node.sub_type() != DeviceSubType::MEDIA_HARD_DRIVE {
            continue;
        }

        // FIXME: is that enough? shouldn't we map _err correctly?
        let hd_path: &HardDrive = node.try_into().map_err(|_err| uefi::Status::UNSUPPORTED)?;
        if let PartitionSignature::Guid(guid) = hd_path.partition_signature() {
            return Ok(guid);
        }
    }

    Err(uefi::Status::UNSUPPORTED.into())
}
