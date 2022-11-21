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

      rust = pkgs.rust-bin.fromRustupToolchainFile ./rust/rust-toolchain.toml;

      naersk' = pkgs.callPackage naersk {
        cargo = rust;
        rustc = rust;
      };

      qemuUefi = pkgs.writeShellScriptBin "qemu-uefi" ''
            exec ${pkgs.qemu}/bin/qemu-system-x86_64 \
              -machine q35,accel=kvm:tcg -bios ${pkgs.OVMF.fd}/FV/OVMF.fd \
              -m 4096 -serial stdio "$@"
          '';

      uefi-run = naersk'.buildPackage {
        src = pkgs.fetchFromGitHub {
          owner = "Richard-W";
          repo = "uefi-run";

          rev = "8ba33c934525458a784a6620705bcf46c3ca91d2";
          sha256 = "fwzWdOinW/ECVI/65pPB1shxPdl2nZThAqlg8wlWg/g=";
        };

        nativeBuildInputs = [ pkgs.makeWrapper ];

        postInstall = ''
          wrapProgram "$out/bin/uefi-run" \
            --add-flags '--bios-path ${pkgs.OVMF.fd}/FV/OVMF.fd --qemu-path ${pkgs.qemu}/bin/qemu-system-x86_64'
        '';
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
      
      lanzaboote = naersk'.buildPackage {
        src = ./rust;
        cargoBuildOptions = old: old ++ [
          "--target x86_64-unknown-uefi"
        ];
      };

      osrel = pkgs.writeText "lanzaboote-osrel" ''
        NAME=Lanzaboote
        VERSION="0"
      '';

      cmdline = pkgs.writeText "lanzaboote-cmdline" "";

      lanzaboote-uki = pkgs.runCommand "lanzboote-uki" {
        nativeBuildInputs = [
          pkgs.binutils-unwrapped
          add-sections
        ];
      } ''
        mkdir -p $out/bin
        add-sections ${lanzaboote}/bin/lanzaboote.efi ${osrel} ${cmdline} $out/bin/lanzaboote.efi
      '';
    in
      rec {
        packages.x86_64-linux = {
          inherit qemuUefi uefi-run lanzaboote lanzaboote-uki;
          default = lanzaboote-uki;
        };

        devShells.x86_64-linux.default = pkgs.mkShell {
          nativeBuildInputs = [
            qemuUefi
            uefi-run
            rust
            pkgs.pev
            add-sections
          ];
        };
      };
}
