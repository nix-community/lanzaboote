use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Clone, Debug)]
pub struct PcrSignatureConfigEntry {
    /// Private key to sign the PCR policies
    #[serde(alias = "privateKeyFile")]
    pub private_key: PathBuf,
    /// Boot phase paths separated by colons (e.g. `enter-initrd`, `enter-initrd:leave-initrd`) to sign a policy for.
    /// If empty, defaults to default phases of `systemd-measure(1)`.
    #[serde(default)]
    pub phases: Vec<String>,
    /// PCR Banks to sign a policy for.
    /// If empty, defaults to default banks of `systemd-measure(1)`.
    #[serde(default)]
    pub banks: Vec<String>,
}

pub fn load_pcr_signature_config(
    pcr_signature_config_path: &Path,
) -> Result<Vec<PcrSignatureConfigEntry>> {
    fs::read(pcr_signature_config_path)
        .context("Failed to read PCR signature config file")
        .and_then(|file| {
            serde_json::from_slice(&file).context("Failed to deserialize PCR signature config")
        })
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct PcrPolicySignatureEntry {
    pub pcrs: Vec<u8>,
    pub pkfp: String,
    pub pol: String,
    pub sig: String,
}

pub type PcrPolicySignature = HashMap<String, Vec<PcrPolicySignatureEntry>>;

/// Combine multiple PCR policy signatures into one (and remove duplicates)
pub fn combine_pcr_policy_signatures(
    policy_signatures: Vec<PcrPolicySignature>,
) -> PcrPolicySignature {
    let mut result = PcrPolicySignature::new();

    for policy_signature in policy_signatures {
        for (bank, measurement_objects) in policy_signature {
            let result_measurement_objects = result.entry(bank).or_default();
            for measurement_object in measurement_objects {
                if !result_measurement_objects.contains(&measurement_object) {
                    result_measurement_objects.push(measurement_object);
                }
            }
        }
    }
    result
}

/// Create a PCR policy signature using `systemd-measure sign` from the PE section files and section data files
pub fn create_pcr_policy_signature_with_systemd_measure(
    systemd: &Path,
    kernel_cmdline_path: &Path,
    kernel_path: &Path,
    initrd_path: &Path,
    os_release_path: &Path,
    pcr_signature_config_path: &Path,
) -> Result<Option<Vec<u8>>> {
    let pcr_signature_config = load_pcr_signature_config(pcr_signature_config_path)?;

    let mut pcr_policy_signatures = Vec::new();

    for pcr_signature_config_entry in pcr_signature_config {
        let mut args = vec![
            OsString::from("sign"),
            OsString::from("--json=short"),
            OsString::from("--cmdline"),
            kernel_cmdline_path.into(),
            OsString::from("--linux"),
            kernel_path.into(),
            OsString::from("--initrd"),
            initrd_path.into(),
            OsString::from("--osrel"),
            os_release_path.into(),
            OsString::from("--private-key"),
            pcr_signature_config_entry.private_key.into(),
        ];

        for phase in pcr_signature_config_entry.phases {
            args.push(OsString::from("--phase"));
            args.push(phase.into());
        }

        for bank in pcr_signature_config_entry.banks {
            args.push(OsString::from("--bank"));
            args.push(bank.into());
        }

        let command = Command::new(format!("{}/lib/systemd/systemd-measure", systemd.display()))
            .args(args)
            .output()
            .context("Failed to run systemd-measure")?;

        pcr_policy_signatures.push(
            serde_json::from_slice::<PcrPolicySignature>(&command.stdout)
                .context("Failed to deserialize PCR policy signature created by systemd-measure")?,
        );
    }

    if !pcr_policy_signatures.is_empty() {
        let pcr_policy_signature = combine_pcr_policy_signatures(pcr_policy_signatures);
        Ok(Some(
            serde_json::to_vec(&pcr_policy_signature)
                .context("Failed to serialize PCR policy signature")?,
        ))
    } else {
        Ok(None)
    }
}
