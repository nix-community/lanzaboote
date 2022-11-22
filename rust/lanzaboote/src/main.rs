#![no_main]
#![no_std]
#![feature(abi_efiapi)]
#![feature(allocator_api)]

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use log::debug;
use uefi::{
    CString16,
    prelude::*,
    proto::{
        console::text::Output,
        media::file::{File, FileAttribute, FileMode, RegularFile, Directory},
    },
    ResultExt
};
use pem_rfc7468::{decode,encoded_len,decode_label};
use ed25519_compact::{PublicKey, Signature};

static mut PUBLIC_KEY_BUFFER: [u8; 256] = [0; 256];
static EMBEDDED_PEM_PUBLIC_KEY: &[u8; 113] = include_bytes!("blitz-public-key.pem");

fn print_logo(output: &mut Output) {
    output.clear().unwrap();

    output
        .output_string(cstr16!(
            "
  _                      _                 _   \r
 | |                    | |               | |  \r
 | | __ _ _ __  ______ _| |__   ___   ___ | |_ \r
 | |/ _` | '_ \\|_  / _` | '_ \\ / _ \\ / _ \\| __|\r
 | | (_| | | | |/ / (_| | |_) | (_) | (_) | |_ \r
 |_|\\__,_|_| |_/___\\__,_|_.__/ \\___/ \\___/ \\__|\r
"
        ))
        .unwrap();
}

fn read_all(image: &mut RegularFile) -> uefi::Result<Vec<u8>> {
    let mut buf = Vec::new();

    // TODO Can we do this nicer?
    loop {
        let mut chunk = [0; 512];
        let read_bytes = image.read(&mut chunk).map_err(|e| e.status())?;

        if read_bytes == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[0..read_bytes]);
    }

    Ok(buf)
}

fn load_public_key() -> pem_rfc7468::Result<PublicKey> {
    let label = decode_label(EMBEDDED_PEM_PUBLIC_KEY)?;
    let len = encoded_len(label, pem_rfc7468::LineEnding::CR, EMBEDDED_PEM_PUBLIC_KEY)?;
    if len > 256 {
        panic!("The vulcano hike was phew")
    }

    let public_key = unsafe { decode(EMBEDDED_PEM_PUBLIC_KEY, &mut PUBLIC_KEY_BUFFER[0..])?.1 };

    // TODO: implement From trait for ed25519-compact errors.
    Ok(PublicKey::from_slice(&public_key[12..]).unwrap())
}

fn with_signature_extension(filename: &CString16) -> Result<CString16, uefi::Error<uefi::data_types::FromStrError>> {
    CString16::try_from(
        format!("{}.sig", filename).as_str()
    ).map_err(|err| uefi::Error::new(uefi::Status::INVALID_PARAMETER, err))
}

fn read_signed_binary(root: &mut Directory, filename: &CString16) -> uefi::Result<(Vec<u8>, Signature)> {
    let mut binary = root
        .open(filename, FileMode::Read, FileAttribute::empty())?
        .into_regular_file().unwrap();
    debug!("Opened the binary");
    let mut signature = root
        .open(&with_signature_extension(filename).discard_errdata()?, FileMode::Read, FileAttribute::empty())?
        .into_regular_file().unwrap();
    debug!("Opened the signature");
    let raw_signature = read_all(&mut signature)?;
    Ok((read_all(&mut binary)?, Signature::new(raw_signature.try_into().unwrap())))
}

#[entry]
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let public_key = load_public_key().unwrap();

    print_logo(system_table.stdout());

    let boot_services = system_table.boot_services();
    let mut file_system = boot_services.get_image_file_system(handle).unwrap();
    let mut root = file_system.open_volume().unwrap();

    debug!("Found root");

    let (binary, expected_signature) = read_signed_binary(&mut root, &CString16::try_from("linux.efi").unwrap()).unwrap();

    let pkey_verification = public_key.verify(&binary, &expected_signature)
        .map_err(|err| panic!("Invalid signature: {:?}", err));

    if pkey_verification.is_ok() {
        debug!("You are allowed to enjoy your computer now");
    }

    let kernel_image = boot_services
        .load_image(
            handle,
            uefi::table::boot::LoadImageSource::FromBuffer {
                buffer: &binary,
                file_path: None,
            },
        )
        .unwrap();

    boot_services.start_image(kernel_image).unwrap();

    Status::SUCCESS
}
