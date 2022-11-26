{
  description = "Lanzaboot Secure Boot Madness";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nixpkgs-test.url = "github:RaitoBezarius/nixpkgs/experimental-secureboot";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, nixpkgs-test, rust-overlay, naersk }:
    let
      pkgs = import nixpkgs {
        system = "x86_64-linux";
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      lib = pkgs.lib;

      rust-nightly = pkgs.rust-bin.fromRustupToolchainFile ./rust/lanzaboote/rust-toolchain.toml;

      naersk-nightly = pkgs.callPackage naersk {
        cargo = rust-nightly;
        rustc = rust-nightly;
      };

      qemuUefi = pkgs.writeShellScriptBin "qemu-uefi" ''
            exec ${pkgs.qemu}/bin/qemu-system-x86_64 \
              -machine q35,accel=kvm:tcg -bios ${pkgs.OVMF.fd}/FV/OVMF.fd \
              -m 4096 -serial stdio "$@"
          '';

      uefi-run = pkgs.callPackage ./nix/uefi-run.nix {
        naersk = naersk-nightly;
      };

      systemd-boot-run = pkgs.writeShellScriptBin "systemd-boot-run" ''
       ${uefi-run}/bin/uefi-run lib/systemd/boot/efi/systemd-bootx64.efi
      '';

      add-sections = pkgs.writeShellScriptBin "add-sections" ''
        set -eu
        IN=$1
        OSREL=$2
        CMDLINE=$3
        OUT=$4

        stub_line=$(objdump -h "$1" | tail -2 | head -1)
        stub_size=0x$(echo "$stub_line" | awk '{print $3}')
        stub_offs=0x$(echo "$stub_line" | awk '{print $4}')
        osrel_offs=$((stub_size + stub_offs))
        cmdline_offs=$((osrel_offs + $(stat -c%s "$OSREL")))
        objcopy \
          --add-section .osrel="$OSREL" --change-section-vma .osrel=$(printf 0x%x $osrel_offs) \
          --add-section .cmdline="$CMDLINE" \
          --change-section-vma .cmdline=$(printf 0x%x $cmdline_offs) \
           "$IN" "$OUT"
      '';

      buildRustEfiApp = src: naersk-nightly.buildPackage {
        inherit src;
        cargoBuildOptions = old: old ++ [
          "--target x86_64-unknown-uefi"
        ];
      };

      buildRustLinuxApp = src: naersk-nightly.buildPackage {
        inherit src;
      };

      # This is basically an empty EFI application that we use as a
      # carrier for the initrd.
      initrd-stub = buildRustEfiApp ./rust/initrd-stub;

      lanzaboote = buildRustEfiApp ./rust/lanzaboote;

      lanzatoolBin = naersk-nightly.buildPackage {
        src = ./rust/lanzatool;
        buildInputs = [ pkgs.binutils ];
      };

      lanzatool = pkgs.runCommand "lanzatool" {
        nativeBuildInputs = [ pkgs.makeWrapper ];
      } ''
        mkdir -p $out/bin

        # Clean PATH to only contain what we need to do objcopy. Also
        # tell lanzatool where to find our UEFI binaries.
        makeWrapper ${lanzatoolBin}/bin/lanzatool $out/bin/lanzatool \
          --set PATH ${lib.makeBinPath [ pkgs.binutils-unwrapped pkgs.sbsigntool ]} \
          --set RUST_BACKTRACE full \
          --set LANZABOOTE_STUB ${lanzaboote}/bin/lanzaboote.efi \
          --set LANZABOOTE_INITRD_STUB ${initrd-stub}/bin/initrd-stub.efi \
      '';

      # A script that takes an initrd and turns it into a PE image.
      wrapInitrd = pkgs.writeShellScriptBin "wrap-initrd" ''
        set -eu

        STUB=${initrd-stub}/bin/initrd-stub.efi
        INITRD=$1
        OUT=$2

        stub_line=$(objdump -h "$STUB" | tail -2 | head -1)
        stub_size=0x$(echo "$stub_line" | awk '{print $3}')
        stub_offs=0x$(echo "$stub_line" | awk '{print $4}')
        initrd_offs=$((stub_size + stub_offs))

        objcopy --add-section .initrd="$INITRD" --change-section-vma .initrd=$(printf 0x%x $initrd_offs) \
          "$STUB" "$OUT"
      '';

      osrel = pkgs.writeText "lanzaboote-osrel" ''
        NAME=Lanzaboote
        VERSION="${lanzaboote.version}"
      '';

      cmdline = pkgs.writeText "lanzaboote-cmdline" "console=ttyS0";

      lanzaboote-uki = pkgs.runCommand "lanzboote-uki" {
        nativeBuildInputs = [
          pkgs.binutils-unwrapped
          add-sections
        ];
      } ''
        mkdir -p $out/bin
        add-sections ${lanzaboote}/bin/lanzaboote.efi ${osrel} ${cmdline} $out/bin/lanzaboote.efi
      '';
    in {
      overlays.default = final: prev: {
        inherit lanzatool;
      };

      nixosModules.lanzaboote = import ./nix/lanzaboote.nix;

      packages.x86_64-linux = {
        inherit qemuUefi uefi-run initrd-stub lanzaboote lanzaboote-uki lanzatool wrapInitrd;
        default = lanzaboote-uki;
      };

      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = [
          qemuUefi
          uefi-run
          lanzatool
          pkgs.openssl
          wrapInitrd
          (pkgs.sbctl.override {
            databasePath = "pki";
          })
          pkgs.sbsigntool
          pkgs.efitools
          pkgs.python39Packages.ovmfvartool
          pkgs.qemu
        ];

        inputsFrom = [
          lanzatoolBin
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
            def extract_bspec_attr(attr):
                return bootspec.get(attr)
            def convert_to_esp(store_file_path):
                store_dir = os.path.basename(os.path.dirname(store_file_path))
                filename = os.path.basename(store_file_path)
                return f'/boot/EFI/nixos/{store_dir}-{filename}.efi'

            machine.start()
            bootspec = json.loads(machine.succeed("cat /run/current-system/bootspec/boot.v1.json"))
            print(machine.succeed("ls /boot/EFI/nixos"))
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
        # TODO: user mode: OK
        # TODO: how to get in: {deployed, audited} mode ?
        lanzaboote-boot = mkSecureBootTest {
          name = "signed-files-boot-under-secureboot";
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
            src = "extract_bspec_attr('initrd')";
            dst = "\"/boot/EFI/nixos/initrd\"";
          };
        };
        is-kernel-secured = mkUnsignedTest {
          name = "unsigned-kernel-do-not-boot-under-secureboot";
          path = {
            src = "extract_bspec_attr('kernel')";
            dst = "\"/boot/EFI/nixos/kernel\"";
          };
        };

      };
    };
}
