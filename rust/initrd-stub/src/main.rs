#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;
use uefi::{
    prelude::{entry, Boot, SystemTable},
    Handle, Status,
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[entry]
fn main(_handle: Handle, mut _system_table: SystemTable<Boot>) -> Status {
    Status::UNSUPPORTED
}
