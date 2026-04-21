# Test that a secret is appended to the initrd during installation. Similar to
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

    testing.initrdBackdoor = true;

    boot.initrd = {
      secrets = {
        "/test" = toString secret;
      };
      systemd.storePaths = [
        "${pkgs.diffutils}/bin/cmp"
      ];
    };
  };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      machine.succeed("${pkgs.diffutils}/bin/cmp ${secret} /test")
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';

}
