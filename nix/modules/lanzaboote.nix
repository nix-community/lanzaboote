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
    enable = mkEnableOption "LANZABOOTE";
    enrollKeys = mkEnableOption "automatic enrollment of the keys using sbctl, DO NOT USE IF YOU DO NOT UNDERSTAND HOW IT WORKS, IT WILL BRICK YOUR MACHINE.";
    unsignedGenerationsPolicy = mkOption {
      type = types.enum [ "resign" "ignore" "resign-last-only" ];
      default = "ignore";
      description = ''
        When introducing SecureBoot in your system, you will most likely have old generations
        which are unsigned and will cease to boot.

        Depending on your threat model, you may want to:

        - resign all of them, ignoring all possibility of existing vulnerabilities in them
        - resign only the last generation, reducing your risk to the very last one, useful if you want to have a known configuration with limited exposure
        - ignore all of them, rendering your rollback feature unusable until SecureBoot is disabled or new generations are introduced
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
          --unsigned-generations-policy ${cfg.unsignedGenerationsPolicy} \
          ${config.boot.loader.efi.efiSysMountPoint} \
          /nix/var/nix/profiles/system-*-link
      '';
    };
  };
}
