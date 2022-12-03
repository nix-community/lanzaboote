{ lib, config, pkgs, ... }: 
with lib;
let
  cfg = config.boot.lanzaboote;
  sbctlWithPki = pkgs.sbctl.override {
    databasePath = "/tmp/pki";
  };
in
{
  options.boot.lanzaboote = {
    enable = mkEnableOption "Enable the LANZABOOTE";
    enrollKeys = mkEnableOption "Automatic enrollment of the keys using sbctl";
    pkiBundle = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "PKI bundle containg db, PK, KEK";
    };
    publicKeyFile = mkOption {
      type = types.path;
      default = if cfg.pkiBundle != null then "${cfg.pkiBundle}/keys/db/db.pem" else null;
      description = "Public key to sign your boot files";
    };
    privateKeyFile = mkOption {
      type = types.path;
      default = if cfg.pkiBundle != null then "${cfg.pkiBundle}/keys/db/db.key" else null;
      description = "Private key to sign your boot files";
    };
    package = mkOption {
      type = types.package;
      default = pkgs.lanzatool;
      description = "Lanzatool package";
    };
  };

  config = mkIf cfg.enable {
    # bootspec is putting at false
    # until we fix this upstream, we will mkForce it.
    boot.loader.supportsInitrdSecrets = mkForce true;
    boot.loader.external = {
      enable = true;
      installHook = pkgs.writeShellScript "bootinstall" ''
        ${optionalString cfg.enrollKeys ''
          mkdir -p /tmp/pki
          cp -r ${cfg.pkiBundle}/* /tmp/pki
          ${sbctlWithPki}/bin/sbctl enroll-keys --yes-this-might-brick-my-machine
        ''}
  
        ${cfg.package}/bin/lanzatool install \
          --public-key ${cfg.publicKeyFile} \
          --private-key ${cfg.privateKeyFile} \
          ${config.boot.loader.efi.efiSysMountPoint} \
          /nix/var/nix/profiles/system-*-link
      '';
    };
  };
}
