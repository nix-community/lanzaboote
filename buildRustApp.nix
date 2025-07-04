{ craneLib
, writeText
, lib
,
}: { pname
   , src
   , target ? null
   , doCheck ? true
   , # By default, it builds the default members of the workspace.
     packages ? null
   , extraArgs ? { }
   ,
   }:
let
  commonArgs =
    {
      inherit pname;
      inherit src;
      CARGO_BUILD_TARGET = target;
      inherit doCheck;

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

        #[cfg_attr(any(target_os = "none", target_os = "uefi"), export_name = "efi_main")]
        fn main() {}
      '';

      cargoExtraArgs =
        (extraArgs.cargoExtraArgs or "")
        + (
          if packages != null
          then (lib.concatStringsSep " " (map (p: "--package ${p}") packages))
          else ""
        );
    }
    // builtins.removeAttrs extraArgs [ "cargoExtraArgs" ];

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  package = craneLib.buildPackage (commonArgs
    // {
    inherit cargoArtifacts;
  });

  clippy = craneLib.cargoClippy (commonArgs
    // {
    inherit cargoArtifacts;
    cargoClippyExtraArgs = "-- --deny warnings";
  });

  rustfmt = craneLib.cargoFmt (commonArgs // { inherit cargoArtifacts; });
}
