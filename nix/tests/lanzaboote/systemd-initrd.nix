{
  name = "lanzaboote-systemd-initrd";

  nodes.machine = { ... }: {
    imports = [ ./common/lanzaboote.nix ];

    boot.initrd.systemd.enable = true;
  };

  testScript = ''
    machine.start()
    assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
  '';
}
