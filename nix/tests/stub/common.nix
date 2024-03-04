{

  virtualisation = {
    useBootLoader = true;
    useEFIBoot = true;
  };

  boot.loader.uki.enable = true;
  boot.loader.efi = {
    canTouchEfiVariables = true;
  };

}
