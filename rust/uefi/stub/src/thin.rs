use log::{error, warn};
use sha2::{Digest, Sha256};
use uefi::{fs::FileSystem, guid, prelude::*, table::runtime::VariableVendor, CString16, Result};

use crate::common::{boot_linux_unchecked, extract_string};
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

pub fn boot_linux(handle: Handle, mut system_table: SystemTable<Boot>) -> uefi::Result<()> {
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

    // The firmware initialized SecureBoot to 1 if performing signature checks, and 0 if it doesn't.
    // Applications are not supposed to modify this variable (in particular, don't change the value from 1 to 0).
    let mut secure_boot_value = [1];
    let secure_boot_result = system_table.runtime_services().get_variable(
        cstr16!("SecureBoot"),
        &VariableVendor(guid!("8be4df61-93ca-11d2-aa0d-00e098032b8c")),
        &mut secure_boot_value,
    );
    let secure_boot = match secure_boot_result {
        // Secure Boot is explicitly disabled, make verification errors non-fatal.
        Ok((&[0], _)) => false,
        // Secure Boot is not supported, make verification errors non-fatal.
        Err(e) if e.status() == Status::NOT_FOUND => false,
        // If Secure Boot is enabled, verification errors must be treated as fatal.
        // To be on the safe side, if anything goes wrong, treat Secure Boot as enabled.
        _ => true,
    };

    if !secure_boot {
        warn!("Secure Boot is not active!");
    }

    let kernel_data;
    let initrd_data;

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

    check_hash(&kernel_data, config.kernel_hash, "Kernel", secure_boot)?;
    check_hash(&initrd_data, config.initrd_hash, "Initrd", secure_boot)?;

    boot_linux_unchecked(
        handle,
        system_table,
        kernel_data,
        &config.cmdline,
        initrd_data,
    )
}
