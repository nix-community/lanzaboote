{ pkgs, runTest, ukiModule }:

let
  common = _: {
    imports = [ ukiModule ];

    virtualisation = {
      useBootLoader = true;
      useEFIBoot = true;
    };

    boot.loader.uki.enable = true;
    boot.loader.efi = {
      canTouchEfiVariables = true;
    };
  };
in
{
  # This test serves as a baseline to make sure that the custom boot installer
  # script defined in the ukiModule works with the upstream systemd-stub. When
  # this test fails something is very wrong.
  systemd-stub = runTest {
    name = "systemd-stub";
    nodes.machine = _: {
      imports = [ common ];
      boot.loader.uki.stub = "${pkgs.systemd}/lib/systemd/boot/efi/linuxx64.efi.stub";
    };
    testScript = ''
      machine.start()
      print(machine.succeed("bootctl status"))
    '';
  };

  fatStub = runTest {
    name = "fat-stub";
    nodes.machine = _: {
      imports = [ common ];
    };
    testScript = ''
      machine.start()
      print(machine.succeed("bootctl status"))
    '';
  };
}
