{
  imports = [ ./image.nix ];

  boot = {
    loader.timeout = 0;
    loader.efi.canTouchEfiVariables = true;

    lanzaboote = {
      enable = true;
      pkiBundle = ../../fixtures/uefi-keys;
    };
  };
}
