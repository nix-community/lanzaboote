{ config, lib, ... }:

let
  pkiBundle = "/var/lib/lanzaboote-test-fixture";
in
{
  imports = [ ./image.nix ];

  options.lanzabooteTest = {
    keyFixture = lib.mkEnableOption "pkiBundle fixture baked into the image" // {
      default = config.virtualisation.useSecureBoot;
    };

    persistentRoot = lib.mkEnableOption "a persistent root filesystem";
  };

  config = {
    systemd.tmpfiles.settings = lib.mkIf config.lanzabooteTest.keyFixture {
      "10-sbctl"."${pkiBundle}".L = {
        argument = "${../../fixtures/uefi-keys}";
      };
    };

    boot = {
      loader.timeout = 0;
      loader.efi.canTouchEfiVariables = true;

      lanzaboote = {
        enable = true;
        pkiBundle = lib.mkDefault pkiBundle;
      };
    };
  };
}
