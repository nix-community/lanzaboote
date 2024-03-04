# Execute a boot test that has an intentionally broken secure boot chain. This
# test is expected to fail with Secure Boot and should succeed without. We test
# this both for an intentionally broken initrd as well as for a broken kernel.

# The initrd is not directly signed. Its hash is embedded into the stub. To
# make integrity verification fail, we actually have to modify the initrd.
# Appending crap to the end is a harmless way that would make the kernel still
# accept it.

{

  name = "lanzaboote-hash-mismatch";

  nodes = {

    brokenInitrd = { lib, ... }: {
      imports = [ ./common/lanzaboote.nix ];
      virtualisation.useSecureBoot = lib.mkForce false;
    };

    brokenInitrdSecureBoot = {
      imports = [ ./common/lanzaboote.nix ];
    };

    brokenKernel = { lib, ... }: {
      imports = [ ./common/lanzaboote.nix ];
      virtualisation.useSecureBoot = lib.mkForce false;
    };

    brokenKernelSecureBoot = {
      imports = [ ./common/lanzaboote.nix ];
    };

  };

  testScript = ''
    initrdGlob = "/boot/EFI/nixos/initrd-*.efi"
    kernelGlob = "/boot/EFI/nixos/kernel-*.efi"

    def prepare(machine, glob):
      machine.start()
      machine.succeed(f"echo some_garbage_to_change_the_hash | tee -a {glob} > /dev/null")
      machine.succeed("sync")
      machine.crash()
      machine.start()

    # Start all VMs simultaneously to save some time
    start_all()

    prepare(brokenInitrd, initrdGlob)
    prepare(brokenInitrdSecureBoot, initrdGlob)
    prepare(brokenKernel, kernelGlob)
    prepare(brokenKernelSecureBoot, kernelGlob)

    brokenInitrd.succeed("bootctl", timeout=120)
    brokenInitrdSecureBoot.wait_for_console_text("hash does not match")
    brokenKernel.succeed("bootctl", timeout=120)
    brokenKernelSecureBoot.wait_for_console_text("hash does not match")
  '';
}
