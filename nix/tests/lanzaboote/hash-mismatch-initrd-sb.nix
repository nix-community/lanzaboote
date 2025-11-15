{

  name = "lanzaboote-hash-mismatch-initrd-sb";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];
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
      machine.wait_for_console_text("hash does not match")
    '';
}
