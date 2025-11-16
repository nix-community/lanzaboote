{
  pkgs,
  crane,
}:

let
  inherit (pkgs) lib;

  toolchain = pkgs.rust-bin.fromRustupToolchainFile ../../rust/uefi/rust-toolchain.toml;
  craneLib = crane.overrideToolchain toolchain;

  buildRustApp = lib.makeOverridable (
    {
      pname,
      src,
      # By default, it builds the default members of the workspace.
      packages ? null,
      # Args applied to all packages (deps, clippy, rustfmt)
      args ? { },
      # Args only applied to the final package, not to the deps
      packageArgs ? { },
    }:
    let
      commonArgs = {
        inherit pname src;
        cargoExtraArgs =
          if packages != null then (lib.concatStringsSep " " (map (p: "--package ${p}") packages)) else "";
      }
      // args;

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      argsWithArtifacts = commonArgs // {
        inherit cargoArtifacts;
      };
    in
    craneLib.buildPackage (
      argsWithArtifacts
      // {
        passthru.tests = {
          clippy = craneLib.cargoClippy (
            argsWithArtifacts
            // {
              cargoClippyExtraArgs = "-- --deny warnings";
            }
          );

          rustfmt = craneLib.cargoFmt argsWithArtifacts;
        };
      }
      // packageArgs
    )
  );
in
rec {
  stub = pkgs.callPackage ./stub.nix { inherit buildRustApp; };
  lzbt = pkgs.callPackage ./lzbt.nix {
    inherit buildRustApp;
    inherit stub;
  };
}
