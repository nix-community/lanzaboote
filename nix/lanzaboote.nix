{ lib, config, pkgs, ... }: 
with lib;
let
  cfg = config.boot.lanzaboote;
in
{
  options.boot.lanzaboote = {
    enable = mkEnableOption "Enable the LANZABOOTE";
    enrollKeys = mkEnableOption "Automatic enrollment of the keys";
    pkiBundle = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "PKI bundle containg db, PK, KEK";
    };
    publicKeyFile = mkOption {
      type = types.path;
      default = if cfg.pkiBundle != null then "${cfg.pkiBundle}/db/db.pem" else null;
      description = "Public key to sign your boot files";
    };
    privateKeyFile = mkOption {
      type = types.path;
      default = if cfg.pkiBundle != null then "${cfg.pkiBundle}/db/db.key" else null;
      description = "Private key to sign your boot files";
    };
    package = mkOption {
      type = types.package;
      default = pkgs.lanzatool;
      description = "Lanzatool package";
    };
  };

  config = mkIf cfg.enable {
    boot.loader.external = {
      enable = true;
      passBootspec = true;
      installHook = if cfg.pkiBundle != null
      then "${cfg.package}/bin/lanzatool install ${optionalString cfg.enrollKeys "--autoenroll"} --pki-bundle ${cfg.pkiBundle}"
      else "${cfg.package}/bin/lanzatool install --public-key ${cfg.publicKeyFile} --private-key ${cfg.privateKeyFile}";
    };
  };
}
