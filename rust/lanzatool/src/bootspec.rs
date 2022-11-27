use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Mapping of specialisation names to their boot.json
    pub specialisation: HashMap<String, Bootspec>,
    pub extension: Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub os_release: PathBuf,
}
