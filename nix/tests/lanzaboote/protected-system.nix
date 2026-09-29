{
  name = "lanzaboote-protected-system";

  nodes = {
    machine = {
      imports = [ ./common/lanzaboote.nix ];

      # We need this so switch-to-configuration exists and can set up /boot2
      system.switch.enable = true;

      boot.lanzaboote.configurationLimit = 2;
    };
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      with subtest("Install second generation"):
        machine.succeed("mkdir -p /nix/var/nix/profiles")
        # We actually use the same toplevel but just create extra symlinks to
        # simulate generations
        machine.succeed("ln -s ${nodes.machine.system.build.toplevel} /nix/var/nix/profiles/system-1-link")
        machine.succeed("ln -s ${nodes.machine.system.build.toplevel} /nix/var/nix/profiles/system-2-link")
        machine.succeed("/run/current-system/bin/switch-to-configuration boot")

        stubs = machine.succeed("ls -1 /boot/EFI/Linux")
        print(stubs)
        t.assertEqual(len(stubs.splitlines()), 2)
        t.assertIn("nixos-generation-1", stubs)
        t.assertIn("nixos-generation-2", stubs)

      with subtest("Install third generation"):
        machine.succeed("ln -s ${nodes.machine.system.build.toplevel} /nix/var/nix/profiles/system-3-link")
        machine.succeed("/run/current-system/bin/switch-to-configuration boot")

        stubs = machine.succeed("ls -1 /boot/EFI/Linux")
        print(stubs)
        t.assertEqual(len(stubs.splitlines()), 2)
        t.assertIn("nixos-generation-1", stubs)
        t.assertIn("nixos-generation-3", stubs)
    '';
}
