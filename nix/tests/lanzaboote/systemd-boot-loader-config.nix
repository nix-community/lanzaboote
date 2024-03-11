{

  name = "lanzaboote-systemd-boot-loader-config";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    boot.loader.timeout = 0;
    boot.loader.systemd-boot.consoleMode = "auto";
  };

  testScript = ''
    machine.start()

    actual_loader_config = machine.succeed("cat /boot/loader/loader.conf").split("\n")
    expected_loader_config = ["timeout 0", "console-mode auto"]

    assert all(cfg in actual_loader_config for cfg in expected_loader_config), \
      f"Expected: {expected_loader_config} is not included in actual config: '{actual_loader_config}'"
  '';

}
