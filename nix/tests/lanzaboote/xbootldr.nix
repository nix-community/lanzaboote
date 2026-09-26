{

  name = "lanzaboote-xbootldr";

  nodes = {
    machine1 = {
      imports = [ ./common/lanzaboote.nix ];

      lanzabooteTest = {
        xbootldr = true;
      };

      boot = {
        loader = {
          systemd-boot.xbootldrMountPoint = "/boot";
          efi.efiSysMountPoint = "/efi";
        };
      };
    };
    machine2 = {
      imports = [ ./common/lanzaboote.nix ];

      lanzabooteTest = {
        xbootldr = true;
      };

      boot = {
        loader = {
          systemd-boot.xbootldrMountPoint = "/boot";
          efi.efiSysMountPoint = "/efi";
        };
        lanzaboote = {
          extraEfiSysMountPoints = [ "/efi2" ];
          extraXbootldrMountPoints = [ "/boot2" ];
        };
      };
      # We need this so switch-to-configuration exists and can set up /boot2
      system.switch.enable = true;
    };
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { machine = nodes.machine1; })
    + (import ./common/image-helper.nix { machine = nodes.machine2; })
    + ''
      # Adapted from basic.nix
      machine1_bootctl_status = machine1.succeed("bootctl status")
      print(machine1_bootctl_status)
      t.assertIn("Secure Boot: enabled (user)", machine1_bootctl_status)
      t.assertIn("ESP: /efi", machine1_bootctl_status)
      t.assertIn("XBOOTLDR: /boot", machine1_bootctl_status)
      t.assertIn("Kernel Type: uki", machine1.succeed("bootctl kernel-inspect /boot/EFI/Linux/nixos-generation-1-*.efi"))

      # Adapted from extra-efi-partitions.nix
      machine2_bootctl_status = machine2.succeed("bootctl status")
      print(machine2_bootctl_status)
      t.assertIn("Secure Boot: enabled (user)", machine2_bootctl_status)
      t.assertIn("ESP: /efi", machine2_bootctl_status)
      t.assertIn("XBOOTLDR: /boot", machine2_bootctl_status)
      machine2.succeed("mkdir -p /nix/var/nix/profiles && ln -s ${nodes.machine2.system.build.toplevel} /nix/var/nix/profiles/system-1-link")
      machine2.succeed("/run/current-system/bin/switch-to-configuration boot")
      t.assertIn("Kernel Type: uki", machine2.succeed("bootctl kernel-inspect /boot/EFI/Linux/nixos-generation-1-*.efi"))
      t.assertIn("Kernel Type: uki", machine2.succeed("bootctl kernel-inspect /boot2/EFI/Linux/nixos-generation-1-*.efi"))
    '';
}
