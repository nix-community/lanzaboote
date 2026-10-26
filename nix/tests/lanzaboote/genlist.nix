{

  name = "lanzaboote-genlist";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];
  };
  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      machine.succeed("bootctl status | grep -q 'initial'")
    '';
}
