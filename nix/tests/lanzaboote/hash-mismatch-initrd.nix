{
  name = "lanzaboote-hash-mismatch-initrd";

  nodes.machine =
    { lib, ... }:
    {
      imports = [ ./common/lanzaboote.nix ];
      virtualisation.useSecureBoot = lib.mkForce false;
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      initrdGlob = "/boot/EFI/nixos/initrd-*.efi"

      machine.start()
      machine.succeed(f"echo some_garbage_to_change_the_hash | tee -a {initrdGlob} > /dev/null")
      machine.succeed("sync")
      machine.crash()

      machine.start()
      machine.succeed("bootctl", timeout=120)
    '';
}
