{ lib, config, pkgs, ... }:
let
  inherit (lib) mkIf;
in
{

  virtualisation = {
    useBootLoader = true;
    useEFIBoot = true;
    useSecureBoot = true;

    efi.OVMF = pkgs.OVMFFull.fd;
  };

  boot = {
    loader.timeout = 0;
    loader.efi.canTouchEfiVariables = true;

    lanzaboote = {
      enable = true;
      safeAutoEnroll = mkIf (config.virtualisation.useSecureBoot) {
        db = ../../fixtures/uefi-keys/keys/db/db.pem;
        KEK = ../../fixtures/uefi-keys/keys/KEK/KEK.pem;
        PK = ../../fixtures/uefi-keys/keys/PK/PK.pem;
      };
      pkiBundle = ../../fixtures/uefi-keys;
    };
  };

}
