{
  name = "lanzaboote-systemd-measure";

  nodes.machine =
    { lib, pkgs, ... }:
    {
      imports = [ ./common/lanzaboote.nix ];

      virtualisation.tpm.enable = true;

      environment.systemPackages = [
        (pkgs.writeShellScriptBin "lanzaboote-measure-uki" ''
          UKI_SECTIONS_DIR=$(mktemp -d)

          for section in .osrel .cmdline; do
            ${lib.getExe' pkgs.bintools "objcopy"} -O binary --only-section=$section /boot/EFI/Linux/nixos-generation-1-*.efi $UKI_SECTIONS_DIR/$section
          done

          /run/current-system/sw/lib/systemd/systemd-measure calculate \
            --linux /boot/EFI/nixos/kernel-*.efi \
            --initrd /boot/EFI/nixos/initrd-*.efi \
            --osrel "$UKI_SECTIONS_DIR/.osrel" \
            --cmdline "$UKI_SECTIONS_DIR/.cmdline" \
            --phase "" \
            --bank "sha256"
        '')
      ];
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      machine.wait_for_unit("default.target")

      pcr_calculated = machine.succeed("lanzaboote-measure-uki")
      pcr_current = machine.succeed("/run/current-system/sw/lib/systemd/systemd-measure status --bank sha256")
      t.assertEqual(pcr_calculated, pcr_current, "Current PCR 11 does not does match")
    '';
}
