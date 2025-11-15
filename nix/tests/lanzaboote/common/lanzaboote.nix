{ config, pkgs, ... }:
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
      enrollKeys = config.virtualisation.useSecureBoot;
      pkiBundle = ../../fixtures/uefi-keys;
    };
  };

}
