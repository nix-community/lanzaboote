let
  sortKey = "mySpecialSortKey";
in
{

  name = "lanzaboote";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    boot.lanzaboote = { inherit sortKey; };
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      bootctl_status = machine.succeed("bootctl status")
      print(bootctl_status)
      t.assertIn("Secure Boot: enabled (user)", bootctl_status)
      t.assertIn("sort-key: ${sortKey}", bootctl_status)

      # We want systemd to recognize our PE binaries as true UKIs. systemd has
      # become more picky in the past, so make sure.
      t.assertIn("Kernel Type: uki", machine.succeed("bootctl kernel-inspect /boot/EFI/Linux/nixos-generation-1-*.efi"))
    '';
}
