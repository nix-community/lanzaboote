{
  name = "lanzaboote-specialisation";

  nodes.machine = { pkgs, ... }: {
    imports = [ ./common/lanzaboote.nix ];

    specialisation.variant.configuration = {
      environment.systemPackages = [
        pkgs.efibootmgr
      ];
    };
  };

  testScript = ''
    machine.start()
    print(machine.succeed("ls -lah /boot/EFI/Linux"))
    # TODO: make it more reliable to find this filename, i.e. read it from somewhere?
    machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant-\*.efi")
    machine.succeed("sync")
    machine.fail("efibootmgr")
    machine.crash()
    machine.start()
    print(machine.succeed("bootctl"))
    # Only the specialisation contains the efibootmgr binary.
    machine.succeed("efibootmgr")
  '';
}
