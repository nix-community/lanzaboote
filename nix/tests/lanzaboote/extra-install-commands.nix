{ pkgs, lib, ... }:
let
  testString = "extraInstallCommandsTest";
  extraInstallCommands = ''
    ${lib.getExe' pkgs.coreutils "mkdir"} -p /boot/custom
    echo '${testString}' > /boot/custom/command_test
  '';
in
{
  name = "lanzaboote-extra-install-commands";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    boot.lanzaboote = { inherit extraInstallCommands; };
  };

  testScript = ''
    machine.start()
    assert "${testString}" == machine.succeed("cat /boot/custom/command_test").strip()
  '';
}
