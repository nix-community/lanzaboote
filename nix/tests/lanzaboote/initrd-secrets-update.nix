# Test that the secrets configured to be appended to the initrd get updated
# when installing a new generation even if the initrd itself (i.e. its store
# path) does not change.
#
# An unfortunate result of this NixOS feature is that updating the secrets
# without creating a new initrd might break previous generations. Verify that
# a new initrd (which is supposed to only differ by the secrets) is created
# in this case.
#
# This tests uses a specialisation to imitate a newer generation. This works
# because `lzbt` installs the specialisation of a generation AFTER installing
# the generation itself (thus making the specialisation "newer").

{ pkgs, ... }:

let

  originalSecret = pkgs.writeText "oh-so-secure" "uhh-ooh-uhh-security";
  newSecret = pkgs.writeText "newly-secure" "so-much-better-now";

in

{
  name = "lanzaboote-initrd-secrets-update";

  nodes.machine = { lib, ... }: {
    imports = [ ./common/lanzaboote.nix ];

    boot.initrd = {
      secrets = {
        "/test" = lib.mkDefault (toString originalSecret);
      };
      postMountCommands = ''
        cp /test /mnt-root/secret-from-initramfs
      '';
    };

    specialisation.variant.configuration = {
      boot.initrd.secrets = {
        "/test" = toString newSecret;
      };
    };
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # Assert that only three boot files exists (a single kernel and a two
    # initrds).
    assert int(machine.succeed("ls -1 /boot/EFI/nixos | wc -l")) == 3

    # It is expected that the initrd contains the original secret.
    machine.succeed("cmp ${originalSecret} /secret-from-initramfs")

    machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant-\*.efi")
    machine.succeed("sync")
    machine.crash()
    machine.start()
    machine.wait_for_unit("multi-user.target")
    # It is expected that the initrd of the specialisation contains the new secret.
    machine.succeed("cmp ${newSecret} /secret-from-initramfs")
  '';

}
