use crate::common::{boot_linux_unchecked, extract_string, get_cmdline, get_secure_boot_status};
use alloc::vec::Vec;
use linux_bootloader::cpio::pack_cpio_literal;
use linux_bootloader::pe_section::pe_section;
use linux_bootloader::uefi_helpers::PeInMemory;
use log::{error, warn};
use sha2::{Digest, Sha256};
use uefi::fs::FileSystem;
use uefi::{CString16, prelude::*};

type Hash = sha2::digest::Output<Sha256>;

/// UKI Components that are load from PE sections or from the file system and are used to boot the system
pub struct UkiComponents {
    /// Kernel data loaded from file system.
    pub kernel_data: Vec<u8>,

    /// Initrd data loaded from file system.
    pub initrd_data: Vec<u8>,

    /// The kernel command-line.
    pub cmdline: CString16,

    /// Raw JSON data with signed PCR values
    pub pcr_signature: Option<Vec<u8>>,
}

/// Extract a SHA256 hash from a PE section.
fn extract_hash(pe_data: &[u8], section: &str) -> uefi::Result<Hash> {
    let array: [u8; 32] = pe_section(pe_data, section)
        .ok_or(Status::INVALID_PARAMETER)?
        .try_into()
        .map_err(|_| Status::INVALID_PARAMETER)?;

    Ok(array.into())
}

impl UkiComponents {
    pub fn load_from_pe(pe_in_memory: &PeInMemory) -> uefi::Result<Self> {
        // SAFETY: We get a slice that represents our currently running
        // image and then parse the PE data structures from it. This is
        // safe, because we don't touch any data in the data sections that
        // might conceivably change while we look at the slice.
        let pe_data = unsafe { pe_in_memory.as_slice() };

        let kernel_filename = extract_string(pe_data, ".linux")?;
        let kernel_hash = extract_hash(pe_data, ".linuxh")?;
        let initrd_filename = extract_string(pe_data, ".initrd")?;
        let initrd_hash = extract_hash(pe_data, ".initrdh")?;
        let cmdline = extract_string(pe_data, ".cmdline")?;

        let file_system = boot::get_image_file_system(boot::image_handle())
            .expect("Failed to get file system handle");
        let mut file_system = FileSystem::new(file_system);

        let (kernel_data, initrd_data);
        kernel_data = file_system
            .read(&*kernel_filename)
            .expect("Failed to read kernel file into memory");
        initrd_data = file_system
            .read(&*initrd_filename)
            .expect("Failed to read initrd file into memory");

        let secure_boot_enabled = get_secure_boot_status();
        check_hash(&kernel_data, kernel_hash, "Kernel", secure_boot_enabled)?;
        check_hash(&initrd_data, initrd_hash, "Initrd", secure_boot_enabled)?;

        Ok(Self {
            kernel_data,
            initrd_data,
            cmdline,
            pcr_signature: pe_section(pe_data, ".pcrsig"),
        })
    }
}

/// Verify some data against its expected hash.
///
/// In case of a mismatch:
/// * If Secure Boot is active, an error message is logged, and the SECURITY_VIOLATION error is returned to stop the boot.
/// * If Secure Boot is not active, only a warning is logged, and the boot process is allowed to continue.
fn check_hash(data: &[u8], expected_hash: Hash, name: &str, secure_boot: bool) -> uefi::Result<()> {
    let hash_correct = Sha256::digest(data) == expected_hash;
    if !hash_correct {
        if secure_boot {
            error!("{name} hash does not match!");
            return Err(Status::SECURITY_VIOLATION.into());
        } else {
            warn!("{name} hash does not match! Continuing anyway.");
        }
    }
    Ok(())
}

pub fn boot_linux(
    handle: Handle,
    components: UkiComponents,
    mut dynamic_initrds: Vec<Vec<u8>>,
) -> uefi::Result<()> {
    let secure_boot_enabled = get_secure_boot_status();
    let cmdline = get_cmdline(&components.cmdline, secure_boot_enabled);

    let mut initrd_data = components.initrd_data;

    if let Some(pcr_signature) = components.pcr_signature {
        let cpio = pack_cpio_literal(
            &pcr_signature,
            uefi::fs::Path::new(cstr16!("tpm2-pcr-signature.json")),
            ".extra",
            555,
            444,
        )
        .expect("Failed to pack cpio from PCR signature data");
        dynamic_initrds.push(cpio.into_inner());
    }

    // Correctness: dynamic initrds are supposed to be validated by caller,
    // i.e. they are system extension images or credentials
    // that are supposedly measured in TPM2.
    // Therefore, it is normal to not verify their hashes against a configuration.

    // Pad to align
    initrd_data.resize(initrd_data.len().next_multiple_of(4), 0);
    for mut extra_initrd in dynamic_initrds {
        // Uncomment for maximal debugging pleasure.
        // let debug_representation = extra_initrd.as_slice().escape_ascii().collect::<Vec<u8>>();
        // log::warn!("{:?}", String::from_utf8_lossy(&debug_representation));
        initrd_data.append(&mut extra_initrd);
        // Extra initrds ideally should be aligned, but just in case, let's verify this.
        initrd_data.resize(initrd_data.len().next_multiple_of(4), 0);
    }

    boot_linux_unchecked(handle, components.kernel_data, &cmdline, initrd_data)
}
