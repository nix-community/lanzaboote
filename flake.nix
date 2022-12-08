{
  description = "Lanzaboot Secure Boot Madness";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-utils.follows = "flake-utils";
    };

    nixpkgs-test.url = "github:RaitoBezarius/nixpkgs/experimental-secureboot";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, nixpkgs-test, rust-overlay, ... }:
    let
      pkgs = import nixpkgs {
        system = "x86_64-linux";
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      lib = pkgs.lib;

      rust-nightly = pkgs.rust-bin.fromRustupToolchainFile ./rust/lanzaboote/rust-toolchain.toml;
      craneLib = crane.lib.x86_64-linux.overrideToolchain rust-nightly;

      uefi-run = pkgs.callPackage ./nix/uefi-run.nix {
        inherit craneLib;
      };

      # Build attributes for a Rust application.
      buildRustApp = {
        src, target ? null, doCheck ? true
      }: let
        cleanedSrc = craneLib.cleanCargoSource src;
        commonArgs = {
          src = cleanedSrc;
          CARGO_BUILD_TARGET = target;
          inherit doCheck;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        package = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        clippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "-- --deny warnings";
        });
      };

      # This is basically an empty EFI application that we use as a
      # carrier for the initrd.
      initrdStubCrane = buildRustApp {
        src = ./rust/initrd-stub;
        target = "x86_64-unknown-uefi";
        doCheck = false;
      };

      lanzabooteCrane = buildRustApp {
        src = ./rust/lanzaboote;
        target = "x86_64-unknown-uefi";
        doCheck = false;
      };

      initrd-stub = initrdStubCrane.package;
      lanzaboote = lanzabooteCrane.package;

      lanzatoolCrane = buildRustApp {
        src = ./rust/lanzatool;
      };

      lanzatool-unwrapped = lanzatoolCrane.package;

      lanzatool = pkgs.runCommand "lanzatool" {
        nativeBuildInputs = [ pkgs.makeWrapper ];
      } ''
        mkdir -p $out/bin

        # Clean PATH to only contain what we need to do objcopy. Also
        # tell lanzatool where to find our UEFI binaries.
        makeWrapper ${lanzatool-unwrapped}/bin/lanzatool $out/bin/lanzatool \
          --set PATH ${lib.makeBinPath [ pkgs.binutils-unwrapped pkgs.sbsigntool ]} \
          --set RUST_BACKTRACE full \
          --set LANZABOOTE_STUB ${lanzaboote}/bin/lanzaboote.efi \
          --set LANZABOOTE_INITRD_STUB ${initrd-stub}/bin/initrd-stub.efi \
      '';
    in {
      overlays.default = final: prev: {
        inherit lanzatool;
      };

      nixosModules.lanzaboote = { pkgs, lib, ... }: {
        imports = [ ./nix/lanzaboote.nix ];
        boot.lanzaboote.package = lib.mkDefault self.packages.${pkgs.system}.lanzaboote;
      };

      packages.x86_64-linux = {
        inherit initrd-stub lanzaboote lanzatool;
        default = lanzatool;
      };

      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = [
          uefi-run
          lanzatool
          pkgs.openssl
          (pkgs.sbctl.override {
            databasePath = "pki";
          })
          pkgs.sbsigntool
          pkgs.efitools
          pkgs.python39Packages.ovmfvartool
          pkgs.qemu
        ];

        inputsFrom = [
          lanzaboote
        ];
      };

      checks.x86_64-linux = let
        mkSecureBootTest = { name, machine ? {}, testScript }: nixpkgs-test.legacyPackages.x86_64-linux.nixosTest {
          inherit name testScript;
          nodes.machine = { lib, ... }: {
            imports = [
              self.nixosModules.lanzaboote
              machine
            ];

            nixpkgs.overlays = [ self.overlays.default ];

            virtualisation = {
              useBootLoader = true;
              useEFIBoot = true;
              useSecureBoot = true;
            };

            boot.loader.efi = {
              enable = true;
              canTouchEfiVariables = true;
            };
            boot.lanzaboote = {
              enable = true;
              enrollKeys = lib.mkDefault true;
              pkiBundle = ./pki;
            };
          };
        };
        mkUnsignedTest = { name, path }: mkSecureBootTest {
          inherit name;
          testScript = ''
            import json
            import os.path
            bootspec = None

            def convert_to_esp(store_file_path):
                store_dir = os.path.basename(os.path.dirname(store_file_path))
                filename = os.path.basename(store_file_path)
                return f'/boot/EFI/nixos/{store_dir}-{filename}.efi'

            machine.start()
            bootspec = json.loads(machine.succeed("cat /run/current-system/bootspec/boot.v1.json"))
            src_path = ${path.src}
            dst_path = ${path.dst}
            machine.succeed(f"cp -rf {src_path} {dst_path}")
            machine.succeed("sync")
            machine.crash()
            machine.start()
            machine.wait_for_console_text("panicked")
          '';
        };
      in
        {
          lanzatool-clippy = lanzatoolCrane.clippy;
          lanzaboote-clippy = lanzabooteCrane.clippy;

          # TODO: user mode: OK
          # TODO: how to get in: {deployed, audited} mode ?
          lanzaboote-boot = mkSecureBootTest {
            name = "signed-files-boot-under-secureboot";
            testScript = ''
              machine.start()
              assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
            '';
          };

          lanzaboote-boot-under-sd-stage1 = mkSecureBootTest {
            name = "signed-files-boot-under-secureboot-systemd-stage-1";
            machine = { ... }: {
              boot.initrd.systemd.enable = true;
            };
            testScript = ''
              machine.start()
              assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
            '';
          };

          # So, this is the responsibility of the lanzatool install
          # to run the append-initrd-secret script
          # This test assert that lanzatool still do the right thing
          # preDeviceCommands should not have any root filesystem mounted
          # so it should not be able to find /etc/iamasecret, other than the
          # initrd's one.
          # which should exist IF lanzatool do the right thing.
          lanzaboote-with-initrd-secrets = mkSecureBootTest {
            name = "signed-files-boot-with-secrets-under-secureboot";
            machine = { ... }: {
              boot.initrd.secrets = {
                "/etc/iamasecret" = (pkgs.writeText "iamsecret" "this is a very secure secret");
              };

              boot.initrd.preDeviceCommands = ''
                grep "this is a very secure secret" /etc/iamasecret
              '';
            };
            testScript = ''
            machine.start()
            assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
          '';
          };
          is-initrd-secured = mkUnsignedTest {
            name = "unsigned-initrd-do-not-boot-under-secureboot";
            path = {
              src = "bootspec.get('initrd')";
              dst = "convert_to_esp(bootspec.get('initrd'))";
            };
          };
          is-kernel-secured = mkUnsignedTest {
            name = "unsigned-kernel-do-not-boot-under-secureboot";
            path = {
              src = "bootspec.get('kernel')";
              dst = "convert_to_esp(bootspec.get('kernel'))";
            };
          };
          specialisation-works = mkSecureBootTest {
            name = "specialisation-still-boot-under-secureboot";
            machine = { pkgs, ... }: {
              specialisation.variant.configuration = {
                environment.systemPackages = [
                  pkgs.efibootmgr
                ];
              };
            };
            testScript = ''
              machine.start()
              print(machine.succeed("ls -lah /boot/EFI/Linux"))
              print(machine.succeed("cat /run/current-system/bootspec/boot.v1.json"))
              # TODO: make it more reliable to find this filename, i.e. read it from somewhere?
              machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant.efi")
              machine.succeed("sync")
              machine.fail("efibootmgr")
              machine.crash()
              machine.start()
              print(machine.succeed("bootctl"))
              # We have efibootmgr in this specialisation.
              machine.succeed("efibootmgr")
            '';
          };
        };
    };
}
