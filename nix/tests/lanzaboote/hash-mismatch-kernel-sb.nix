{

  name = "lanzaboote-hash-mismatch-kernel-sb";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      kernelGlob = "/boot/EFI/nixos/kernel-*.efi"

      machine.start()
      machine.succeed(f"echo some_garbage_to_change_the_hash | tee -a {kernelGlob} > /dev/null")
      machine.succeed("sync")
      machine.crash()

      machine.start()
      machine.wait_for_console_text("hash does not match")
    '';
}
