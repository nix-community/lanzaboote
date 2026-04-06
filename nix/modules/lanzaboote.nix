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

  mkInstallCommand =
    efiSysMountPoint:
    ''
      PATH=${config.systemd.package}/lib/systemd:$PATH
      ${cfg.installCommand} \
    ''
    + (
      lib.escapeShellArgs (
        [
          "--public-key=${toString cfg.publicKeyFile}"
          "--private-key=${toString cfg.privateKeyFile}"
        ]
        ++ lib.optionals (cfg.measuredBoot.enable && pcr 4) [
          "--pcrlock-directory=${cfg.measuredBoot.pcrlockDirectory}"
        ]
        ++ [
          efiSysMountPoint
        ]
      )
      + " /nix/var/nix/profiles/system-*-link"
    );

  format = pkgs.formats.yaml { };
  sbctlConfigFile = format.generate "sbctl.conf" {
    keydir = "${cfg.pkiBundle}/keys";
    guid = "${cfg.pkiBundle}/GUID";
  };

  json = pkgs.formats.json { };

  pcr = n: lib.elem n cfg.measuredBoot.pcrs;

  staticMeasurements = pkgs.runCommand "pcrlock.d" { preferLocalBuild = true; } ''
    mkdir -p $out

    for f in ${toString cfg.measuredBoot.upstreamStaticMeasurements}; do
      mkdir -p $(dirname $out/$f)
      ln -sf ${config.systemd.package}/lib/pcrlock.d/$f $out/$f
    done

    ${lib.concatLines (
      lib.mapAttrsToList (n: v: "ln -s ${v.source} $out/${n}.pcrlock") cfg.measuredBoot.staticMeasurements
    )}
  '';

  makePolicyCommand = lib.escapeShellArgs (
    [
      "${config.systemd.package}/lib/systemd/systemd-pcrlock"
      "make-policy"
      "--components=${staticMeasurements}"
      "--components=${cfg.measuredBoot.pcrlockDirectory}"
      "--policy=${cfg.measuredBoot.pcrlockPolicy}"
      "--location=770"
    ]
    ++ lib.map (pcr: "--pcr=${toString pcr}") cfg.measuredBoot.pcrs
  );
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

    logLevel = lib.mkOption {
      type = lib.types.enum [
        "info"
        "debug"
      ];
      default = "info";
      description = ''
        Log level of lzbt.
      '';
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
        ${lib.getExe cfg.package} ${lib.optionalString (cfg.logLevel == "debug") "-vv"} install \
          --system ${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          --systemd ${config.systemd.package} \
          --systemd-boot-loader-config ${loaderConfigFile} \
          --configuration-limit ${toString configurationLimit} \
          --allow-unsigned ${lib.boolToString cfg.allowUnsigned} \
          --bootcounting-initial-tries ${toString cfg.bootCounting.initialTries}'';
      defaultText = lib.literalExpression ''
        ''${lib.getExe config.boot.lanzaboote.package} ''${lib.optionalString (config.boot.lanzaboote.logLevel == "debug") "-vv"} install \
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

    measuredBoot = {
      enable = lib.mkEnableOption "Measured Boot";

      pcrs = lib.mkOption {
        type = lib.types.listOf (
          lib.types.enum [
            0
            1
            2
            3
            4
            7
          ]
        );
        default = [ ];
        description = ''
          PCRs to lock via systemd-pcrlock.
        '';
      };

      pcrlockDirectory = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/pcrlock.d";
        description = ''
          Directory to store the pcrlock files in.
        '';
      };

      pcrlockPolicy = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/systemd/pcrlock.json";
        description = ''
          Location to store the pcrlock policy in.
        '';
      };

      upstreamStaticMeasurements = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = ''
          Filenames of static pcrlock measurements to include from the systemd
          package.
        '';
      };

      staticMeasurements = lib.mkOption {
        default = { };
        description = ''
          Static systemd-pcrlock measurements.
        '';
        type = lib.types.attrsOf (
          lib.types.submodule (
            {
              name,
              config,
              options,
              ...
            }:
            {
              options = {
                source = lib.mkOption {
                  type = lib.types.path;
                  description = "Path of the source file.";
                };
                json = lib.mkOption {
                  default = null;
                  type = lib.types.nullOr json.type;
                  description = ''
                    systemd-pcrlock components in their literal form. This option is directly transformed to a JSON.
                  '';
                };
              };
              config = {
                source = lib.mkIf (config.json != null) (
                  lib.mkDerivedConfig options.json (json.generate "${name}.pcrlock")
                );
              };
            }
          )
        );
      };

      autoCryptenroll = {
        enable = lib.mkEnableOption "automatically re-enroll systemd-pcrlock TPM2 policy into LUKS volume";

        device = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          description = ''
            The device that is encrypted via LUKS2 to enroll the TPM2 policy into.

            This is useful for unattended systems to upgrade a LUKS2 volume
            from being locked against a static PCR to a full systemd-pcrlock
            policy.
          '';
        };

        autoReboot = lib.mkEnableOption "" // {
          description = ''
            Whether to automatically reboot after preparing the measurements.

            Enable this to enroll the new systemd-pcrlock policy with full
            protection without having to wait for a manual reboot.
          '';
        };
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
      {
        assertion = cfg.measuredBoot.enable -> (configurationLimit > 0 && configurationLimit <= 8);
        message = ''
          If Measured Boot is enabled, you cannot store more than 8 generations on the ESP.

            This is a strict limit required and enforced by systemd-pcrlock.

            Set `boot.lanzaboote.configurationLimit = 8;` to reduce the number of generations you store.
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
        ''
          ${lib.concatStringsSep "\n" (map mkInstallCommand efiSysMountPoints)}
        ''
        + lib.optionalString cfg.measuredBoot.enable ''
          echo "Predicting the PCR state for future boots..."
          ${makePolicyCommand}
        ''
      );
    };
    boot.lanzaboote.measuredBoot.upstreamStaticMeasurements =
      lib.optionals (pcr 0 || pcr 1 || pcr 2 || pcr 3 || pcr 4) [
        "500-separator.pcrlock.d/300-0x00000000.pcrlock"
      ]
      ++ lib.optionals (pcr 4) [
        "350-action-efi-application.pcrlock"
      ]
      ++ lib.optionals (pcr 7) [
        "400-secureboot-separator.pcrlock.d/300-0x00000000.pcrlock"
      ];

    environment.etc."sbctl/sbctl.conf" =
      lib.mkIf (cfg.autoGenerateKeys.enable || cfg.autoEnrollKeys.enable)
        {
          source = sbctlConfigFile;
        };

    # Write this to /etc so that manually calling systemd-pcrlock by the user
    # still works without them having to specify the directory.
    environment.etc."pcrlock" = lib.mkIf cfg.measuredBoot.enable {
      target = "pcrlock.d";
      source = staticMeasurements;
    };

    systemd.additionalUpstreamSystemUnits = lib.mkIf cfg.measuredBoot.enable [
      "systemd-pcrlock-make-policy.service"
      "systemd-pcrlock-firmware-code.service"
      "systemd-pcrlock-firmware-config.service"
      "systemd-pcrlock-secureboot-policy.service"
      "systemd-pcrlock-secureboot-authority.service"
    ];
    systemd.services.systemd-pcrlock-make-policy = lib.mkIf cfg.measuredBoot.enable {
      wantedBy = [ "sysinit.target" ];

      serviceConfig.ExecStart = [
        "" # unset previous value
        makePolicyCommand
      ];
    };
    systemd.targets.sysinit = lib.mkIf cfg.measuredBoot.enable {
      wants =
        lib.optionals (pcr 0 || pcr 2) [
          "systemd-pcrlock-firmware-code.service"
        ]
        ++ lib.optionals (pcr 1 || pcr 3) [
          "systemd-pcrlock-firmware-config.service"
        ]
        ++ lib.optionals (pcr 7) [
          "systemd-pcrlock-secureboot-policy.service"
          "systemd-pcrlock-secureboot-authority.service"
        ];
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
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
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

    # Re-create/sign all artifacts on the ESP to securely generate pcrlock
    # measurements. If this is successful, immediately reboot. Only on the next
    # boot can the new policy be created and enrolled because the artifacts
    # which were used for the current boot might not be the ones used after
    # re-creating/signing them. systemd-pcrlock only allows including PCRs if
    # the current TPM measurements are included.
    systemd.services.prepare-auto-cryptenroll = lib.mkIf cfg.measuredBoot.autoCryptenroll.enable {
      wantedBy = [ "sysinit.target" ];
      before = [ "systemd-pcrlock-make-policy.service" ];
      # Allow creating files for re-creating/signing via systemd-tmpfiles.
      # This is useful for testing and shouldn't harm anything.
      after = [ "systemd-tmpfiles-setup.service" ];

      unitConfig = {
        DefaultDependencies = false;
        # Only run once before any policy was created because this only needs
        # to be done once when enabling Measured Boot for the first time. On
        # subsequent boots, this is handled via lzbt in the bootinstall hook of
        # switch-to-configuration.
        ConditionPathExists = [ "!${cfg.measuredBoot.pcrlockPolicy}" ];
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };

      script = ''
        ${mkInstallCommand espMountPoint}
      '';
    };

    systemd.services.auto-cryptenroll = lib.mkIf cfg.measuredBoot.autoCryptenroll.enable {
      wantedBy = [ "multi-user.target" ];
      after = [ "prepare-auto-cryptenroll.service" ];

      unitConfig = {
        ConditionPathExists = [
          "${cfg.measuredBoot.pcrlockPolicy}"
          # If this path exists the new policy was already enrolled and thus
          # does not need to be enrolled again. systemd-pcrlock will update the
          # policy in place in the same NV index of the TPM.
          "!/var/lib/auto-cryptenroll/1"
        ];
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        StateDirectory = "auto-cryptenroll";
        ExecStart = ''
          systemd-cryptenroll \
            --wipe-slot=tpm2 \
            --tpm2-device=auto \
            --unlock-tpm2-device=auto \
            --tpm2-pcrlock=${cfg.measuredBoot.pcrlockPolicy} \
            ${cfg.measuredBoot.autoCryptenroll.device}
        '';
        ExecStartPost = "${pkgs.coreutils}/bin/touch /var/lib/auto-cryptenroll/1";
      };
    };

    # Reboot in a separate service instead of via SuccessAction= in individual
    # services so that users can both autoEnrollKeys and autoCryptenroll.
    systemd.services.auto-reboot =
      lib.mkIf (cfg.autoEnrollKeys.autoReboot || cfg.measuredBoot.autoCryptenroll.autoReboot)
        {
          requiredBy = [
            "prepare-sb-auto-enroll.service"
            "prepare-auto-cryptenroll.service"
          ];
          after = [
            "prepare-sb-auto-enroll.service"
            "prepare-auto-cryptenroll.service"
          ];

          serviceConfig = {
            Type = "oneshot";
            ExecStart = "systemctl reboot";
          };
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
