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

    # To get that elusive green checkmark from GNOME Device Security /
    # fwupdtool security.
    lockdown = mkEnableOption "Security Lockdown";

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
          --pki-bundle ${cfg.pkiBundle} \
          --public-key ${cfg.publicKeyFile} \
          --private-key ${cfg.privateKeyFile} \
          ${config.boot.loader.efi.efiSysMountPoint} \
          /nix/var/nix/profiles/system-*-link
      '';
    };

    boot.kernelParams = mkIf cfg.lockdown [ "mem_encrypt=on" ];

    boot.kernelPatches = mkIf cfg.lockdown [
      {
        # The thinklmi driver is violating the sysfs spec and fwupd
        # really wants its 'type' field in the firmware attributes.
        name = "think-lmi-fwupd-compat";
        patch = ./patches/linux/0001-platform-x86-think-lmi-expose-type-attribute.patch;

        # TODO Userspace must still enable lockdown via /sys/kernel/security/lockdown or via command line.
        extraConfig = ''
          AMD_MEM_ENCRYPT y

          SECURITY_LOCKDOWN_LSM y
          SECURITY_LOCKDOWN_LSM_EARLY y
        '';
      }
    ];
  };
}
