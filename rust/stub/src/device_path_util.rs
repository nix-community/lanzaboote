use uefi::{cstr16, Result, proto::device_path::DevicePath, CString16};

// FIXME: should this be upstreamed to uefi-rs?
pub fn device_path_to_str(dp: &DevicePath) -> Result<CString16> {
}
