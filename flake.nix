{
  description = "Lanzaboot Secure Boot Madness";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
    nixpkgs-test.url = "github:RaitoBezarius/nixpkgs/simplified-qemu-boot-disks";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-utils.follows = "flake-utils";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, nixpkgs-test, crane, rust-overlay, ... }:
    let
      pkgs = import nixpkgs {
        system = "x86_64-linux";
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      testPkgs = import nixpkgs-test { system = "x86_64-linux"; };

      inherit (pkgs) lib;

      rust-nightly = pkgs.rust-bin.fromRustupToolchainFile ./rust/lanzaboote/rust-toolchain.toml;
      craneLib = crane.lib.x86_64-linux.overrideToolchain rust-nightly;

      uefi-run = pkgs.callPackage ./nix/packages/uefi-run.nix {
        inherit craneLib;
      };

      # Build attributes for a Rust application.
      buildRustApp =
        { src
        , target ? null
        , doCheck ? true
        , extraArgs ? { }
        }:
        let
          commonArgs = {
            inherit src;
            CARGO_BUILD_TARGET = target;
            inherit doCheck;
          } // extraArgs;

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        in
        {
          package = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
          });

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "-- --deny warnings";
          });
        };

      lanzabooteCrane = buildRustApp {
        src = craneLib.cleanCargoSource ./rust/lanzaboote;
        target = "x86_64-unknown-uefi";
        doCheck = false;
      };

      lanzaboote = lanzabooteCrane.package;

      lanzatoolCrane = buildRustApp {
        src = ./rust/lanzatool;
        extraArgs = {
          TEST_SYSTEMD = pkgs.systemd;
          checkInputs = with pkgs; [
            binutils-unwrapped
            sbsigntool
          ];
        };
      };

      lanzatool-unwrapped = lanzatoolCrane.package;

      lanzatool = pkgs.runCommand "lanzatool"
        {
          nativeBuildInputs = [ pkgs.makeWrapper ];
        } ''
        mkdir -p $out/bin

        # Clean PATH to only contain what we need to do objcopy. Also
        # tell lanzatool where to find our UEFI binaries.
        makeWrapper ${lanzatool-unwrapped}/bin/lanzatool $out/bin/lanzatool \
          --set PATH ${lib.makeBinPath [ pkgs.binutils-unwrapped pkgs.sbsigntool ]} \
          --set RUST_BACKTRACE full \
          --set LANZABOOTE_STUB ${lanzaboote}/bin/lanzaboote.efi
      '';
    in
    {
      overlays.default = final: prev: {
        inherit lanzatool;
      };

      nixosModules.lanzaboote = { pkgs, lib, ... }: {
        imports = [ ./nix/modules/lanzaboote.nix ];
        boot.lanzaboote.package = lib.mkDefault self.packages.${pkgs.system}.lanzatool;
      };

      packages.x86_64-linux = {
        inherit lanzaboote lanzatool;
        default = lanzatool;
      };

      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = [
          uefi-run
          pkgs.openssl
          (pkgs.sbctl.override {
            databasePath = "pki";
          })
          pkgs.sbsigntool
          pkgs.efitools
          pkgs.python39Packages.ovmfvartool
          pkgs.qemu
          pkgs.nixpkgs-fmt
          pkgs.statix
        ];

        inputsFrom = [
          lanzaboote
          lanzatool
        ];

        TEST_SYSTEMD = pkgs.systemd;
      };

      checks.x86_64-linux = {
        lanzatool-clippy = lanzatoolCrane.clippy;
        lanzaboote-clippy = lanzabooteCrane.clippy;
      } // (import ./nix/tests/lanzaboote.nix {
        inherit pkgs testPkgs;
        lanzabooteModule = self.nixosModules.lanzaboote;
      });
    };
}
