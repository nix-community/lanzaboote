{

  name = "fat-stub";

  nodes.machine = _: {
    imports = [ ./common.nix ];
  };

  testScript = ''
    machine.start()
    print(machine.succeed("bootctl status"))
  '';

}
