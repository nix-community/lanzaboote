{
  lib,
  buildRustApp,
  stdenv,
  writeText,
}:

buildRustApp {
  pname = "lanzaboote-stub";
  src = lib.sourceFilesBySuffices ../../rust/uefi [
    ".rs"
    ".toml"
    ".lock"
  ];
  args = {
    CARGO_BUILD_TARGET = "${stdenv.hostPlatform.qemuArch}-unknown-uefi";
    doCheck = false;

    # Workaround for https://github.com/ipetkov/crane/issues/262.
    dummyrs = writeText "dummy.rs" ''
      #![allow(unused)]

      #![cfg_attr(
        any(target_os = "none", target_os = "uefi"),
        no_std,
        no_main,
      )]

      #[cfg_attr(any(target_os = "none", target_os = "uefi"), panic_handler)]
      fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
          loop {}
      }

      #[cfg_attr(any(target_os = "none", target_os = "uefi"), unsafe(export_name = "efi_main"))]
      fn main() {}
    '';
  };
}
