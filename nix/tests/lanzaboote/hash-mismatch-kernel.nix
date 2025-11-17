# Execute a boot test that has an intentionally broken secure boot chain. This
# test is expected to fail with Secure Boot and should succeed without. We test
# this both for an intentionally broken initrd as well as for a broken kernel.

# The initrd is not directly signed. Its hash is embedded into the stub. To
# make integrity verification fail, we actually have to modify the initrd.
# Appending crap to the end is a harmless way that would make the kernel still
# accept it.
{
  name = "lanzaboote-hash-mismatch-kernel";

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
      kernelGlob = "/boot/EFI/nixos/kernel-*.efi"

      machine.start()
      machine.succeed(f"echo some_garbage_to_change_the_hash | tee -a {kernelGlob} > /dev/null")
      machine.succeed("sync")
      machine.crash()

      machine.start()
      machine.succeed("bootctl", timeout=120)
    '';
}
