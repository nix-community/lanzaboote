# required sources
# nixpkgs
# crane: https://github.com/ipetkov/crane.git
# rust-overlay: https://github.com/oxalica/rust-overlay.git
{ sources # set this to null if explicitly passing in sources i.e. without npins
, nixpkgs ? sources.nixpkgs
, rust-overlay ? sources.rust-overlay
, pkgs ? import nixpkgs {
    overlays = [ (import rust-overlay) ];
  }
, crane ? sources.crane
}:
let
  inherit (pkgs.lib) makeOverridable makeBinPath;
  uefi-rust-stable = pkgs.rust-bin.fromRustupToolchainFile ./rust/uefi/rust-toolchain.toml;
  craneLib = (pkgs.callPackage (crane + "/lib") { }).overrideToolchain uefi-rust-stable;
  rustTarget = "${pkgs.stdenv.hostPlatform.qemuArch}-unknown-uefi";
  buildRustApp = makeOverridable (pkgs.callPackage ./buildRustApp.nix {
    inherit craneLib;
  });

  stubCrane = buildRustApp {
    pname = "lanzaboote-stub";
    src = craneLib.cleanCargoSource ./rust/uefi;
    target = rustTarget;
    doCheck = false;
  };

  stub = stubCrane.package;

  toolCrane = buildRustApp {
    pname = "lzbt-systemd";
    src = ./rust/tool;
    extraArgs = {
      TEST_SYSTEMD = pkgs.systemd;
      nativeCheckInputs = with pkgs; [
        binutils-unwrapped
        sbsigntool
      ];
    };
  };

  tool = toolCrane.package;

  wrappedTool =
    pkgs.runCommand "lzbt"
      {
        nativeBuildInputs = [ pkgs.makeWrapper ];
        meta.mainProgram = "lzbt";
      } ''
      mkdir -p $out/bin
      makeWrapper ${tool}/bin/lzbt-systemd $out/bin/lzbt \
        --set PATH ${makeBinPath [pkgs.binutils-unwrapped pkgs.sbsigntool]} \
        --set LANZABOOTE_STUB ${stub}/bin/lanzaboote_stub.efi
    '';
in
pkgs.lib.fix (self: {
  packages = {
    inherit stub;
    tool = wrappedTool;
    lzbt = wrappedTool;
  };

  nixosModules.lanzaboote = { ... }: {
    imports = [ ./nix/modules/lanzaboote.nix ];
    boot.lanzaboote.package = self.packages.tool;
  };
})
