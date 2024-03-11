# Test that a secret is appended to the initrd during installation. Smilar to
# the initrd-secrets test in Nixpkgs:
# https://github.com/NixOS/nixpkgs/blob/master/nixos/tests/initrd-secrets.nix

{ pkgs, ... }:

let

  secret = (pkgs.writeText "oh-so-secure" "uhh-ooh-uhh-security");

in

{

  name = "lanzaboote-initrd-secrets";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    boot.initrd = {
      secrets = {
        "/test" = toString secret;
      };
      postMountCommands = ''
        cp /test /mnt-root/secret-from-initramfs
      '';
    };
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    machine.succeed("cmp ${secret} /secret-from-initramfs")
    assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
  '';

}
