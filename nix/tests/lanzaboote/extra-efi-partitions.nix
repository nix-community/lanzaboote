{

  name = "lanzaboote-extra-efi-partitions";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    boot.lanzaboote = {
      extraEfiSysMountPoints = [ "/boot2" ];
    };
    # We need this so switch-to-configuration exists and can set up /boot2
    system.switch.enable = true;
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      # Prepare secondary "ESP" with files
      machine.succeed("mkdir -p /nix/var/nix/profiles && ln -s ${nodes.machine.system.build.toplevel} /nix/var/nix/profiles/system-1-link")
      machine.succeed("/run/current-system/bin/switch-to-configuration boot")
      # Similar to basic.nix test, except ensure we have copies of the relevant files on both EFI partitions.
      t.assertIn("Kernel Type: uki", machine.succeed("bootctl kernel-inspect /boot/EFI/Linux/nixos-generation-1-*.efi"))
      t.assertIn("Kernel Type: uki", machine.succeed("bootctl kernel-inspect /boot2/EFI/Linux/nixos-generation-1-*.efi"))
    '';
}
