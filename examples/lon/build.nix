let
  sources = import ./lon.nix;
  pkgs = import sources.nixpkgs { };
in
pkgs.nixos [
  ./configuration.nix
  {
    fileSystems."/".device = "/dev/sdX";
  }
]
