# Test installing Lanzaboote without Bootspec support.

# FIXME: currently broken on aarch64
#> mkfs.fat 4.2 (2021-01-31)
#> setting up /etc...
#> Enrolling keys to EFI variables...✓
#> Enrolled keys to the EFI variables!
#> Installing Lanzaboote to "/boot"...
#> No bootable generations found! Aborting to avoid unbootable system. Please check for Lanzaboote updates!
#> [ 2.788390] reboot: Power down

{

  name = "lanzaboote-synthesis";

  nodes.machine = { lib, ... }: {
    imports = [ ./common/lanzaboote.nix ];

    boot.bootspec.enable = lib.mkForce false;
  };

  testScript = ''
    machine.start()
    assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
  '';

}
