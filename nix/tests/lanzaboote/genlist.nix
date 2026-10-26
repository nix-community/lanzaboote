{

  name = "lanzaboote-genlist";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];
  };
  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      bootctl_status = machine.succeed("bootctl status")
      print(bootctl_status)
    '';
}
