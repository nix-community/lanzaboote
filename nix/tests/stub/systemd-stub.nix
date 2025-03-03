# This test serves as a baseline to make sure that the custom boot installer
# script defined in the ukiModule works with the upstream systemd-stub. When
# this test fails something is very wrong.

{

  name = "systemd-stub";

  nodes.machine = { pkgs, ... }: {
    imports = [ ./common.nix ];
  };

  testScript = ''
    machine.start()
    print(machine.succeed("bootctl status"))
  '';

}

