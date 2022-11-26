use std::path::PathBuf;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Bootspec {
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
    /// Hashmap of <Specialisation Name, Bootspec>
    pub specialisation: HashMap<String, Bootspec>
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub esp: String,
    pub bootctl: PathBuf,
    pub os_release: PathBuf,
    pub systemd: PathBuf,
}
