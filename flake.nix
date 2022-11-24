{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, rust-overlay, naersk }:
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
          --set PATH ${lib.makeBinPath [ pkgs.binutils-unwrapped ]} \
          --set LANZABOOTE_STUB ${lanzaboote}/bin/lanzaboote.efi \
          --set LANZABOOTE_INITRD_STUB ${initrd-stub}/bin/initrd-stub.efi
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
        ];

        inputsFrom = [
          lanzatool
          lanzaboote
        ];
      };
    };
}
