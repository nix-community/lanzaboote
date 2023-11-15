{ lib, config, pkgs, ... }:
with lib;
let
  cfg = config.boot.lanzaboote;

  sbctlWithPki = pkgs.sbctl.override {
    databasePath = "/tmp/pki";
  };

  loaderSettingsFormat = pkgs.formats.keyValue {
    mkKeyValue = k: v: if v == null then "" else
    lib.generators.mkKeyValueDefault { } " " k v;
  };

  assembleCredentialDirectoryFromDrv = drv: ''
    for credential in ${drv}/*; do
      echo "Processing $credential"
      if [[ "''${credential##*.}" != "cred" ]]; then
        echo "Found a non-credential: $credential, please remove it or move it."
        exit 1
      fi
      cp $credential $out/
    done
  '';

  assembleCredentialDirectory = drvs: pkgs.runCommand "assemble-credentials" { } ''
    mkdir -p $out/
    ${concatStringsSep "\n" (map assembleCredentialDirectoryFromDrv drvs)}
  '';

  globalCredentialsDirectory = assembleCredentialDirectory cfg.globalCredentials;
  localCredentialsDirectory = assembleCredentialDirectory cfg.localCredentials;

  loaderConfigFile = loaderSettingsFormat.generate "loader.conf" cfg.settings;

  configurationLimit = if cfg.configurationLimit == null then 0 else cfg.configurationLimit;
in
{
  options.boot.lanzaboote = {
    enable = mkEnableOption "Enable the LANZABOOTE";

    enrollKeys = mkEnableOption "Automatic enrollment of the keys using sbctl";

    configurationLimit = mkOption {
      default = config.boot.loader.systemd-boot.configurationLimit;
      defaultText = "config.boot.loader.systemd-boot.configurationLimit";
      example = 120;
      type = types.nullOr types.int;
      description = lib.mdDoc ''
        Maximum number of latest generations in the boot menu.
        Useful to prevent boot partition running out of disk space.

        `null` means no limit i.e. all generations
        that were not garbage collected yet.
      '';
    };

    globalCredentials = mkOption {
      type = types.listOf types.package;
      description = lib.mdDoc ''
        A list of derivations containing multiple .cred files inside of it.
        If anything else than a .cred is found, in the top-level, this will fail
        at assembly time.

        This will be installed in $ESP/loader/credentials.
        In case of data conflict, the installer will fail and ask for manual removal.
      '';
    };

    localCredentials = mkOption {
      type = types.listOf types.package;
      description = lib.mdDoc ''
        A list of derivations containing multiple .cred files inside of it.
        If anything else than a .cred is found, in the top-level, this will fail
        at assembly time.

        This will be installed in this generation's drop-in directory specifically.
        In case of data conflict, the installer will fail and ask for manual removal.
      '';
    };

    pkiBundle = mkOption {
      type = types.nullOr types.path;
      description = "PKI bundle containing db, PK, KEK";
    };

    publicKeyFile = mkOption {
      type = types.path;
      default = "${cfg.pkiBundle}/keys/db/db.pem";
      defaultText = "\${cfg.pkiBundle}/keys/db/db.pem";
      description = "Public key to sign your boot files";
    };

    privateKeyFile = mkOption {
      type = types.path;
      default = "${cfg.pkiBundle}/keys/db/db.key";
      defaultText = "\${cfg.pkiBundle}/keys/db/db.key";
      description = "Private key to sign your boot files";
    };

    package = mkOption {
      type = types.package;
      default = pkgs.lzbt;
      defaultText = "pkgs.lzbt";
      description = "Lanzaboote tool (lzbt) package";
    };

    settings = mkOption rec {
      type = types.submodule {
        freeformType = loaderSettingsFormat.type;
      };

      apply = recursiveUpdate default;

      default = {
        timeout = config.boot.loader.timeout;
        console-mode = config.boot.loader.systemd-boot.consoleMode;
        editor = config.boot.loader.systemd-boot.editor;
        default = "nixos-*";
      };

      defaultText = ''
        {
          timeout = config.boot.loader.timeout;
          console-mode = config.boot.loader.systemd-boot.consoleMode;
          editor = config.boot.loader.systemd-boot.editor;
          default = "nixos-*";
        }
      '';

      example = literalExpression ''
        {
          editor = null; # null value removes line from the loader.conf
          beep = true;
          default = "@saved";
          timeout = 10;
        }
      '';

      description = ''
        Configuration for the `systemd-boot`

        See `loader.conf(5)` for supported values.
      '';
    };
  };

  config = mkIf cfg.enable {
    boot.bootspec = {
      enable = true;
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

        # Use the system from the kernel's hostPlatform because this should
        # always, even in the cross compilation case, be the right system.
        ${cfg.package}/bin/lzbt install \
          --system ${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          --systemd ${config.systemd.package} \
          --systemd-boot-loader-config ${loaderConfigFile} \
          --public-key ${cfg.publicKeyFile} \
          --private-key ${cfg.privateKeyFile} \
          --configuration-limit ${toString configurationLimit} \
          --global-credentials-directory ${globalCredentialsDirectory} \
          --local-credentials-directory ${localCredentialsDirectory} \
          ${config.boot.loader.efi.efiSysMountPoint} \
          /nix/var/nix/profiles/system-*-link
      '';
    };

    systemd.services.fwupd = lib.mkIf config.services.fwupd.enable {
      # Tell fwupd to load its efi files from /run
      environment.FWUPD_EFIAPPDIR = "/run/fwupd-efi";
    };

    systemd.services.fwupd-efi = lib.mkIf config.services.fwupd.enable {
      description = "Sign fwupd EFI app";
      # Exist with the lifetime of the fwupd service
      wantedBy = [ "fwupd.service" ];
      partOf = [ "fwupd.service" ];
      before = [ "fwupd.service" ];
      # Create runtime directory for signed efi app
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        RuntimeDirectory = "fwupd-efi";
      };
      # Place the fwupd efi files in /run and sign them
      script = ''
        ln -sf ${config.services.fwupd.package.fwupd-efi}/libexec/fwupd/efi/fwupd*.efi /run/fwupd-efi/
        ${pkgs.sbsigntool}/bin/sbsign --key '${cfg.privateKeyFile}' --cert '${cfg.publicKeyFile}' /run/fwupd-efi/fwupd*.efi
      '';
    };

    services.fwupd.uefiCapsuleSettings = lib.mkIf config.services.fwupd.enable {
      DisableShimForSecureBoot = true;
    };
  };
}
