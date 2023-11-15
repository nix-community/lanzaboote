use alloc::vec;
use alloc::vec::Vec;
use log::{error, warn};
use sha2::{Digest, Sha256};
use uefi::{fs::FileSystem, prelude::*, CString16, Result};

use crate::common::{boot_linux_unchecked, extract_string, get_cmdline, get_secure_boot_status};
use linux_bootloader::pe_section::pe_section;
use linux_bootloader::uefi_helpers::booted_image_file;

type Hash = sha2::digest::Output<Sha256>;

/// The configuration that is embedded at build time.
///
/// After this stub is built, lzbt needs to embed configuration into the binary by adding PE
/// sections. This struct represents that information.
struct EmbeddedConfiguration {
    /// The filename of the kernel to be booted. This filename is
    /// relative to the root of the volume that contains the
    /// lanzaboote binary.
    kernel_filename: CString16,

    /// The cryptographic hash of the kernel.
    kernel_hash: Hash,

    /// The filename of the initrd to be passed to the kernel. See
    /// `kernel_filename` for how to interpret these filenames.
    initrd_filename: CString16,

    /// The cryptographic hash of the initrd. This hash is computed
    /// over the whole PE binary, not only the embedded initrd.
    initrd_hash: Hash,

    /// The kernel command-line.
    cmdline: CString16,
}

/// Extract a SHA256 hash from a PE section.
fn extract_hash(pe_data: &[u8], section: &str) -> Result<Hash> {
    let array: [u8; 32] = pe_section(pe_data, section)
        .ok_or(Status::INVALID_PARAMETER)?
        .try_into()
        .map_err(|_| Status::INVALID_PARAMETER)?;

    Ok(array.into())
}

impl EmbeddedConfiguration {
    fn new(file_data: &[u8]) -> Result<Self> {
        Ok(Self {
            kernel_filename: extract_string(file_data, ".kernelp")?,
            kernel_hash: extract_hash(file_data, ".kernelh")?,

            initrd_filename: extract_string(file_data, ".initrdp")?,
            initrd_hash: extract_hash(file_data, ".initrdh")?,

            cmdline: extract_string(file_data, ".cmdline")?,
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
    mut system_table: SystemTable<Boot>,
    dynamic_initrds: Vec<Vec<u8>>,
) -> uefi::Result<()> {
    uefi_services::init(&mut system_table).unwrap();

    // SAFETY: We get a slice that represents our currently running
    // image and then parse the PE data structures from it. This is
    // safe, because we don't touch any data in the data sections that
    // might conceivably change while we look at the slice.
    let config = unsafe {
        EmbeddedConfiguration::new(
            booted_image_file(system_table.boot_services())
                .unwrap()
                .as_slice(),
        )
        .expect("Failed to extract configuration from binary. Did you run lzbt?")
    };

    let secure_boot_enabled = get_secure_boot_status(system_table.runtime_services());

    let kernel_data;
    let mut initrd_data;

    {
        let file_system = system_table
            .boot_services()
            .get_image_file_system(handle)
            .expect("Failed to get file system handle");
        let mut file_system = FileSystem::new(file_system);

        kernel_data = file_system
            .read(&*config.kernel_filename)
            .expect("Failed to read kernel file into memory");
        initrd_data = file_system
            .read(&*config.initrd_filename)
            .expect("Failed to read initrd file into memory");
    }

    let cmdline = get_cmdline(
        &config.cmdline,
        system_table.boot_services(),
        secure_boot_enabled,
    );

    check_hash(
        &kernel_data,
        config.kernel_hash,
        "Kernel",
        secure_boot_enabled,
    )?;
    check_hash(
        &initrd_data,
        config.initrd_hash,
        "Initrd",
        secure_boot_enabled,
    )?;

    // Correctness: dynamic initrds are supposed to be validated by caller,
    // i.e. they are system extension images or credentials
    // that are supposedly measured in TPM2.
    // Therefore, it is normal to not verify their hashes against a configuration.

    /// Compute the necessary padding based on the provided length
    /// It returns None if no padding is necessary.
    fn compute_pad4(len: usize) -> Option<Vec<u8>> {
        let overhang = len % 4;
        if overhang != 0 {
            let repeat = 4 - overhang;
            Some(vec![0u8; repeat])
        } else {
            None
        }
    }
    if let Some(mut padding) = compute_pad4(initrd_data.len()) {
        initrd_data.append(&mut padding);
    }

    for mut extra_initrd in dynamic_initrds {
        // Uncomment for maximal debugging pleasure.
        // let debug_representation = extra_initrd.as_slice().escape_ascii().collect::<Vec<u8>>();
        // log::warn!("{:?}", String::from_utf8_lossy(&debug_representation));
        initrd_data.append(&mut extra_initrd);
        // Extra initrds ideally should be aligned, but just in case, let's verify this.
        if let Some(mut padding) = compute_pad4(initrd_data.len()) {
            initrd_data.append(&mut padding);
        }
    }

    boot_linux_unchecked(handle, system_table, kernel_data, &cmdline, initrd_data)
}
