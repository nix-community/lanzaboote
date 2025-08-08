{
  lib,
  config,
  options,
  pkgs,
  ...
}:
let
  cfg = config.boot.lanzaboote;
  espMountPoint = config.boot.loader.efi.efiSysMountPoint;

  loaderSettingsFormat = pkgs.formats.keyValue {
    mkKeyValue = k: v: if v == null then "" else lib.generators.mkKeyValueDefault { } " " k v;
  };

  loaderConfigFile = loaderSettingsFormat.generate "loader.conf" cfg.settings;

  configurationLimit = if cfg.configurationLimit == null then 0 else cfg.configurationLimit;

  efiSysMountPoints = [
    espMountPoint
  ]
  ++ cfg.extraEfiSysMountPoints;

  mkInstallCommand = efiSysMountPoint: ''
    prof_dir="/nix/var/nix/profiles" profiles=""

    # Add all default system profiles, if any
    if [ "$(${lib.getExe pkgs.findutils} "$prof_dir" -type l -name 'system-*-link' \
            | ${lib.getExe' pkgs.coreutils "wc"} -l)" -gt 0 ]; then
      profiles+="$prof_dir/system-*-link "
    fi

    # Add all extra system profiles, if any
    if [ -d "$prof_dir/system-profiles" ] &&
        [ "$(${lib.getExe pkgs.findutils} \
               "$prof_dir/system-profiles" -type l -name '*-*-link' \
             | ${lib.getExe' pkgs.coreutils "wc"} -l)" -gt 0 ]; then
      profiles+="$prof_dir/system-profiles/*-*-link"
    fi

    # Display a clear error message if no usable profile can be found
    if [ "$profiles" = "" ]; then
      ${lib.getExe' pkgs.coreutils "printf"} "%s %s %s\n" \
        "Failed to find usable system profiles. Please make sure that" \
        "system profiles are present under $prof_dir/system-*-link and/or" \
        "$prof_dir/system-profiles/<profile-name>-*-link." 1>&2
      exit 1
    fi

    ${cfg.installCommand} \
      --public-key ${cfg.publicKeyFile} \
      --private-key ${cfg.privateKeyFile} \
      ${efiSysMountPoint} \
      $profiles
  '';

  format = pkgs.formats.yaml { };
  sbctlConfigFile = format.generate "sbctl.conf" {
    keydir = "${cfg.pkiBundle}/keys";
    guid = "${cfg.pkiBundle}/GUID";
  };
in
{
  imports = [
    (lib.mkRemovedOptionModule [ "boot" "lanzaboote" "enrollKeys" ] ''
      Removed this internal option intended for testig only without replacement.
    '')
  ];

  options.boot.lanzaboote = {
    enable = lib.mkEnableOption "Enable the LANZABOOTE";

    configurationLimit = lib.mkOption {
      default = config.boot.loader.systemd-boot.configurationLimit;
      defaultText = "config.boot.loader.systemd-boot.configurationLimit";
      example = 120;
      type = lib.types.nullOr lib.types.int;
      description = ''
        Maximum number of latest generations in the boot menu.
        Useful to prevent boot partition running out of disk space.

        `null` means no limit i.e. all generations
        that were not garbage collected yet.
      '';
    };

    pkiBundle = lib.mkOption {
      type = lib.types.nullOr lib.types.externalPath;
      description = "PKI bundle containing db, PK, KEK";
    };

    publicKeyFile = lib.mkOption {
      type = lib.types.path;
      default = "${cfg.pkiBundle}/keys/db/db.pem";
      defaultText = "\${config.boot.lanzaboote.pkiBundle}/keys/db/db.pem";
      description = "Public key to sign your boot files";
    };

    privateKeyFile = lib.mkOption {
      type = lib.types.path;
      default = "${cfg.pkiBundle}/keys/db/db.key";
      defaultText = "\${config.boot.lanzaboote.pkiBundle}/keys/db/db.key";
      description = "Private key to sign your boot files";
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.lzbt;
      defaultText = lib.literalExpression "pkgs.lzbt";
      description = "Lanzaboote tool (lzbt) package";
    };

    settings = lib.mkOption {
      type = lib.types.submodule {
        freeformType = loaderSettingsFormat.type;
      };

      apply = lib.recursiveUpdate options.boot.lanzaboote.settings.default;

      default = {
        timeout = config.boot.loader.timeout;
        console-mode = config.boot.loader.systemd-boot.consoleMode;
        editor = config.boot.loader.systemd-boot.editor;
        default = "nixos-*";
      }
      // lib.optionalAttrs cfg.autoEnrollKeys.enable {
        secure-boot-enroll = "force";
      };

      defaultText = ''
        {
          timeout = config.boot.loader.timeout;
          console-mode = config.boot.loader.systemd-boot.consoleMode;
          editor = config.boot.loader.systemd-boot.editor;
          default = "nixos-*";
        }
        // lib.optionalAttrs config.boot.lanzaboote.autoEnrollKeys.enable {
          secure-boot-enroll = "force";
        };
      '';

      example = lib.literalExpression ''
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

    sortKey = lib.mkOption {
      default = "lanza";
      type = lib.types.str;
      description = ''
        The sort key used for the NixOS bootloader entries. This key determines
        sorting relative to non-NixOS entries. See also
        https://uapi-group.org/specifications/specs/boot_loader_specification/#sorting
      '';
    };

    bootCounting = {
      initialTries = lib.mkOption {
        type = lib.types.ints.u32;
        default = 0;
        description = ''
          The number of boot counting tries to set for new boot entries.
          Setting this to zero, disables boot counting.
          See https://systemd.io/AUTOMATIC_BOOT_ASSESSMENT/
        '';
      };
    };

    installCommand = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      description = ''
        The partial command to execute lzbt install. This can be used to build
        images by adding the directory to install to and the path to the
        toplevel.
      '';
      default = ''
        # Use the system from the kernel's hostPlatform because this should
        # always, even in the cross compilation case, be the right system.
        ${lib.getExe cfg.package} install \
          --system ${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          --systemd ${config.systemd.package} \
          --systemd-boot-loader-config ${loaderConfigFile} \
          --configuration-limit ${toString configurationLimit} \
          --allow-unsigned ${lib.boolToString cfg.allowUnsigned} \
          --bootcounting-initial-tries ${toString cfg.bootCounting.initialTries}'';
      defaultText = lib.literalExpression ''
        ''${lib.getExe config.boot.lanzaboote.package} install \
          --system ''${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          --systemd ''${config.systemd.package} \
          --systemd-boot-loader-config ''${loaderConfigFile} \
          --configuration-limit ''${toString configurationLimit} \
          --allow-unsigned ''${lib.boolToString config.boot.lanzaboote.allowUnsigned} \
          --bootcounting-initial-tries ''${toString config.boot.lanzaboote.bootCounting.initialTries}'';
    };

    extraEfiSysMountPoints = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = ''
        List of EFI system partition mount points to install the bootloader to (additionally to boot.loader.efi.efiSysMountPoint).
      '';
      default = [ ];
    };

    allowUnsigned = lib.mkEnableOption "" // {
      description = ''
        Whether to allow installing unsigned artifacts to the ESP.

        This is useful for installing Lanzaboote where the key is generated during the first boot.
      '';
      default = cfg.autoGenerateKeys.enable;
      defaultText = "config.boot.lanzaboote.autoGenerateKeys.enable";
    };

    autoGenerateKeys = {
      enable = lib.mkEnableOption "automatically generating Secure Boot keys if they do not exist";
    };

    autoEnrollKeys = {
      enable = lib.mkEnableOption "" // {
        description = "Whether to automatically enroll the Secure Boot keys.";
      };

      autoReboot = lib.mkEnableOption "" // {
        description = ''
          Whether to automatically reboot after preparing the keys for auto enrollment.

          Enable this to enroll the keys via systemd-boot into the firmware
          right after they have been provisioned without waiting for a manual reboot.
        '';
      };

      includeMicrosoftKeys = lib.mkEnableOption "" // {
        description = "Whether to include Microsoft keys when enrolling the Secure Boot keys.";
        default = true;
      };

      includeChecksumsFromTPM = lib.mkEnableOption "" // {
        description = "Whether to include checksums from the TPM Eventlog when enrolling the Secure Boot keys.";
      };

      allowBrickingMyMachine = lib.mkEnableOption "" // {
        description = ''
          Whether to ignore option ROM signatures when enrolling the Secure
          Boot keys. This might brick your machine. Be sure you know what
          you're doing before enabling this.

          See <https://github.com/Foxboron/sbctl/wiki/FAQ#option-rom> for more
          details.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = !cfg.autoEnrollKeys.allowBrickingMyMachine -> cfg.autoEnrollKeys.includeMicrosoftKeys;
        message = ''
          You have set potentially dangerous Secure Boot enrollment settings. This might brick your machine.

            You have two options:
            1. Include the Microsoft keys via autoEnrollKeys.includeMicrosoftKeys
            2. Accept the risk via autoEnrollKeys.allowBrickingMyMachine
        '';
      }
    ];

    boot.bootspec = {
      enable = true;
      extensions."org.nix-community.lanzaboote" = {
        sort_key = config.boot.lanzaboote.sortKey;
      };
    };
    boot.loader.supportsInitrdSecrets = true;
    boot.loader.external = {
      enable = true;
      installHook = pkgs.writeShellScript "bootinstall" (
        lib.concatStringsSep "\n" (map mkInstallCommand efiSysMountPoints)
      );
    };

    environment.etc."sbctl/sbctl.conf" =
      lib.mkIf (cfg.autoGenerateKeys.enable || cfg.autoEnrollKeys.enable)
        {
          source = sbctlConfigFile;
        };

    systemd.services.generate-sb-keys = lib.mkIf cfg.autoGenerateKeys.enable {
      wantedBy = [ "multi-user.target" ];

      unitConfig = {
        # Check to make sure keys directory is not present. Needs to check for
        # a subdirectory of pkiBundle as typically in impermanence-based configs
        # pkiBundle will be persisted, so it will always exist and is not
        # a true determination of whether keys have been generated previously.
        ConditionPathExists = "!${cfg.pkiBundle}/keys";
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.sbctl}/bin/sbctl create-keys";
      };
    };

    # Generate the EFI Authenticated Variables from the keys using sbctl, place
    # them on the ESP, and re-sign all artifacts on the ESP with Lanzaboote.
    # The actual enrollment of the keys into the firmware is done on the next
    # boot via systemd-boot.
    systemd.services.prepare-sb-auto-enroll = lib.mkIf cfg.autoEnrollKeys.enable {
      wantedBy = [ "multi-user.target" ];
      after = [ "generate-sb-keys.service" ];

      unitConfig = {
        ConditionPathExists = [
          "!${espMountPoint}/loader/keys/auto/PK.auth"
          "!${espMountPoint}/loader/keys/auto/KEK.auth"
          "!${espMountPoint}/loader/keys/auto/db.auth"
        ];
        SuccessAction = lib.mkIf cfg.autoEnrollKeys.autoReboot "reboot";
      };

      serviceConfig = {
        Type = "oneshot";
        # SuccessAction doesn't trigger if the service is RemainAfterExit
        RemainAfterExit = lib.mkIf (!cfg.autoEnrollKeys.autoReboot) true;
        RuntimeDirectory = "prepare-sb-auto-enroll";
        WorkingDirectory = "/run/prepare-sb-auto-enroll";
      };

      script =
        let
          sbctlArgs = lib.concatStringsSep " " (
            [ "--export auth" ]
            ++ lib.optionals cfg.autoEnrollKeys.includeMicrosoftKeys [ "--microsoft" ]
            ++ lib.optionals cfg.autoEnrollKeys.includeChecksumsFromTPM [ "--tpm-eventlog" ]
            ++ lib.optionals cfg.autoEnrollKeys.allowBrickingMyMachine [
              "--yes-this-might-brick-my-machine"
            ]
          );
        in
        ''
          ${pkgs.sbctl}/bin/sbctl enroll-keys ${sbctlArgs}

          mkdir -p ${espMountPoint}/loader/keys/auto
          install {PK,KEK,db}.auth ${espMountPoint}/loader/keys/auto/

          # Re-sign all the artifacts on the ESP after the new keys have been
          # auto enrolled.
          ${mkInstallCommand espMountPoint}
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
        ${lib.getExe' pkgs.sbsigntool "sbsign"} --key '${cfg.privateKeyFile}' --cert '${cfg.publicKeyFile}' /run/fwupd-efi/fwupd*.efi
      '';
    };

    services.fwupd.uefiCapsuleSettings = lib.mkIf config.services.fwupd.enable {
      DisableShimForSecureBoot = true;
    };
  };
}
