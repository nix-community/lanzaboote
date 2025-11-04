let
  sortKey = "mySpecialSortKey";
  sortKeySpecialisation = "variantSortKey";
  # Sort key that should put this boot entry in the beginning
  sortKeySpecialisation2 = "avariantSortKey";
in
{
  name = "lanzaboote-specialisation";

  nodes = {
    machine = { lib, pkgs, ... }: {
      imports = [ ./common/lanzaboote.nix ];

      boot.lanzaboote = { inherit sortKey; };

      specialisation.variant.configuration = {
        boot.lanzaboote.sortKey = lib.mkForce sortKeySpecialisation;
        environment.systemPackages = [
          pkgs.efibootmgr
        ];
      };
    };

    machine2 = { lib, pkgs, ... }: {
      imports = [ ./common/lanzaboote.nix ];

      boot.lanzaboote = { inherit sortKey; };

      specialisation.variant.configuration = {
        boot.lanzaboote.sortKey = lib.mkForce sortKeySpecialisation2;
        environment.systemPackages = [
          pkgs.efibootmgr
        ];
      };
    };
  };

  testScript = # python
    ''
      start_all()

      print(machine.succeed("ls -lah /boot/EFI/Linux"))

      out = machine.succeed("bootctl status")
      assert "sort-key: ${sortKey}" in out, "did not find sort key for machine2"

      # TODO: make it more reliable to find this filename, i.e. read it from somewhere?
      machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant-\*.efi")
      machine.succeed("sync")
      machine.fail("efibootmgr")
      machine.crash()
      machine.start()
      print(machine.succeed("bootctl"))

      out = machine.succeed("bootctl status")
      assert "sort-key: ${sortKeySpecialisation}" in out, "did not find specialisation sort key for machine"

      # Only the specialisation contains the efibootmgr binary.
      machine.succeed("efibootmgr")

      # The second machine should have booted into the specialisation
      out = machine2.succeed("bootctl status")
      print(out)
      assert "sort-key: ${sortKeySpecialisation2}" in out, "did not find specialisation sort key for machine2"
      machine2.succeed("efibootmgr")
    '';
}
