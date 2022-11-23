{ lib, config, ... }: 
with lib;
let
  cfg = config.boot.lanzaboote;
in
{
  options.boot.lanzaboote = {
    enable = mkEnableOption "Enable the LANZABOOTE";
  };

  config = mkIf cfg.enable {
    boot.loader.external = {
      enable = true;
      installHook = "${pkgs.lanzatool}/bin/lanzatool install";
    };
  };
}
