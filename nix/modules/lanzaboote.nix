{ lib, config, pkgs, ... }:
with lib;
let
  cfg = config.boot.lanzaboote;
  sbctlWithPki = pkgs.sbctl.override {
    databasePath = "/tmp/pki";
  };

  configurationLimit = if cfg.configurationLimit == null then 0 else cfg.configurationLimit;
in
{
  options.boot.lanzaboote = {
    enable = mkEnableOption "Enable the LANZABOOTE";
    enrollKeys = mkEnableOption "Automatic enrollment of the keys using sbctl";
    configurationLimit = mkOption {
      default = null;
      example = 120;
      type = types.nullOr types.int;
      description = lib.mdDoc ''
        Maximum number of latest generations in the boot menu.
        Useful to prevent boot partition running out of disk space.
        `null` means no limit i.e. all generations
        that were not garbage collected yet.
      '';
    };
    pkiBundle = mkOption {
      type = types.nullOr types.path;
      description = "PKI bundle containg db, PK, KEK";
    };
    publicKeyFile = mkOption {
      type = types.path;
      default = "${cfg.pkiBundle}/keys/db/db.pem";
      description = "Public key to sign your boot files";
    };
    privateKeyFile = mkOption {
      type = types.path;
      default = "${cfg.pkiBundle}/keys/db/db.key";
      description = "Private key to sign your boot files";
    };
    package = mkOption {
      type = types.package;
      default = pkgs.lanzatool;
      description = "Lanzatool package";
    };
  };

  config = mkIf cfg.enable {
    boot.bootspec = {
      enable = true;
      extensions."lanzaboote"."osRelease" = config.environment.etc."os-release".source;
    };
    boot.loader.supportsInitrdSecrets = true;
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
          --configuration-limit ${toString configurationLimit} \
          ${config.boot.loader.efi.efiSysMountPoint} \
          /nix/var/nix/profiles/system-*-link
      '';
    };
  };
}
