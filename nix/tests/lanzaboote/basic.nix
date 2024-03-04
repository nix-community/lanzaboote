{

  name = "lanzaboote";


  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];
  };

  testScript = ''
    machine.start()
    assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")

    # We want systemd to recognize our PE binaries as true UKIs. systemd has
    # become more picky in the past, so make sure.
    assert "Kernel Type: uki" in machine.succeed("bootctl kernel-inspect /boot/EFI/Linux/nixos-generation-1-*.efi")
  '';
}
