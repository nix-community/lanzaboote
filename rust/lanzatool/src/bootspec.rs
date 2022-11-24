use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bootspec {
    pub v1: GenerationV1,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenerationV1 {
    /// Label for the system closure
    pub label: String,
    /// Path to kernel (bzImage) -- $toplevel/kernel
    pub kernel: PathBuf,
    /// list of kernel parameters
    pub kernel_params: Vec<String>,
    /// Path to the init script
    pub init: PathBuf,
    /// Path to initrd -- $toplevel/initrd
    pub initrd: PathBuf,
    /// Path to "append-initrd-secrets" script -- $toplevel/append-initrd-secrets
    pub initrd_secrets: Option<PathBuf>,
    /// config.system.build.toplevel path
    pub toplevel: PathBuf,
    pub extension: Extension,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub esp: String,
    pub bootctl: PathBuf,
    pub os_release: PathBuf,
    pub systemd: PathBuf,
}
