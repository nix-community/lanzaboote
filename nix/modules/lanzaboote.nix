{
  lib,
  config,
  options,
  pkgs,
  utils,
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
        ++ lib.optionals (cfg.protectedSystem != null) [
          "--protected-system=${cfg.protectedSystem}"
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

  installHook = pkgs.writeShellScriptBin "lzbt" (
    ''
      ${lib.concatStringsSep "\n" (map mkInstallCommand efiSysMountPoints)}
    ''
    + lib.optionalString cfg.measuredBoot.enable ''
      echo "Predicting the PCR state for future boots..."
      ${makePolicyCommand}
    ''
  );

  secretsArgs = lib.optionalString cfg.encryptedSecrets.enable (
    lib.concatStringsSep " " [
      "--secrets-private-key=${cfg.encryptedSecrets.privateKeyFile}"
      "--secrets-public-key=${cfg.encryptedSecrets.publicKeyFile}"
    ]
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

  encryptSecrets = cfg.encryptedSecrets.enable && config.boot.initrd.secrets != { };

  # Where the ciphertexts live inside the appended initrd archive. This is the
  # same location the stock appender uses, so the stock copy service copies the
  # ciphertexts (and the public key) into place before they are decrypted.
  encryptedSecretsRoot = "/.initrd-secrets";
  # The public key of the signed PCR policy has to be available inside the initrd
  # for verification.
  initrdPublicKeyPath = "/etc/systemd/tpm2-pcr-public-key.pem";

  # The initrd compressor must match the one used for the stock initrd, so that the
  # appended secrets form a valid, equally compressed cpio archive.
  compressorMeta = import "${pkgs.path}/pkgs/build-support/kernel/initrd-compressor-meta.nix";
  compressorExe =
    if lib.isFunction config.boot.initrd.compressor then
      config.boot.initrd.compressor pkgs
    else if
      !builtins.hasContext config.boot.initrd.compressor
      && builtins.hasAttr config.boot.initrd.compressor compressorMeta
    then
      compressorMeta.${config.boot.initrd.compressor}.executable pkgs
    else
      config.boot.initrd.compressor;
  compressorName = builtins.elemAt (builtins.match "([^ ]*/)?([^ ]+).*" compressorExe) 1;
  compressorArgs =
    if config.boot.initrd.compressorArgs == null then
      compressorMeta.${compressorName}.defaultArgs or [ ]
    else
      config.boot.initrd.compressorArgs;

  # Encrypts every `boot.initrd.secrets` entry against a signed PCR 11 policy and
  # appends only the ciphertext (plus the public key needed for verification) to
  # the initrd.
  encryptSecretsScript = pkgs.writeShellApplication {
    name = "append-encrypted-initrd-secrets";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.cpio
      pkgs.findutils
      pkgs.gnugrep
      pkgs.openssl
      config.systemd.package
    ];
    text = ''
      if [ $# -ne 1 ]; then
        echo "USAGE: $0 INITRD_FILE" >&2
        echo "Appends this configuration's TPM-encrypted secrets to INITRD_FILE" >&2
        exit 1
      fi

      tmp=$(mktemp -d "''${TMPDIR:-/tmp}/initrd-secrets.XXXXXXXXXX")
      trap 'rm -rf "$tmp"' EXIT

      # systemd-creds needs an RSA public key PEM for its signed PCR policy, while
      # the configured file may be an X.509 certificate (or a private key).
      mkdir -p "$tmp${encryptedSecretsRoot}${lib.dirOf initrdPublicKeyPath}"
      public_key="$tmp${encryptedSecretsRoot}${initrdPublicKeyPath}"
      if grep -q -- "BEGIN CERTIFICATE" ${lib.escapeShellArg (toString cfg.encryptedSecrets.publicKeyFile)}; then
        openssl x509 -in ${lib.escapeShellArg (toString cfg.encryptedSecrets.publicKeyFile)} -pubkey -noout > "$public_key"
      elif grep -qE -- "BEGIN (RSA )?PRIVATE KEY" ${lib.escapeShellArg (toString cfg.encryptedSecrets.publicKeyFile)}; then
        openssl pkey -in ${lib.escapeShellArg (toString cfg.encryptedSecrets.publicKeyFile)} -pubout > "$public_key"
      else
        cp ${lib.escapeShellArg (toString cfg.encryptedSecrets.publicKeyFile)} "$public_key"
      fi

      ${lib.concatStringsSep "\n" (
        lib.mapAttrsToList (
          dest: source:
          let
            relative = lib.removePrefix "/" dest;
            source' = if source == null then dest else toString source;
          in
          ''
            mkdir -p "$tmp${encryptedSecretsRoot}/$(dirname ${lib.escapeShellArg relative})"
            systemd-creds encrypt \
              --with-key=tpm2-with-public-key \
              --tpm2-device=auto \
              --tpm2-public-key="$public_key" \
              --tpm2-public-key-pcrs=11 \
              --name=${lib.escapeShellArg (utils.escapeSystemdPath dest)} \
              ${lib.escapeShellArg source'} \
              "$tmp"${encryptedSecretsRoot}/${lib.escapeShellArg relative}
          ''
        ) config.boot.initrd.secrets
      )}

      (cd "$tmp" && find . -mindepth 1 -print0 | xargs -0 touch -amt 197001010000 && find . -mindepth 1 -print0 | sort -z | cpio --quiet -o -H newc -R +0:+0 --reproducible --null) | \
        ${compressorExe} ${lib.escapeShellArgs compressorArgs} >> "$1"
    '';
  };

  # Decrypts the ciphertexts copied into the initrd back to their destinations.
  # Runs as part of the initrd secret copy service, so that units ordered after it
  # find the plaintext secrets in place.
  decryptSecretsScript = pkgs.writeShellApplication {
    name = "decrypt-initrd-secrets";
    runtimeInputs = [
      pkgs.coreutils
      config.systemd.package
    ];
    text = ''
      umask 0077

      [ -d ${encryptedSecretsRoot} ] || exit 0

      ${lib.concatStringsSep "\n" (
        lib.mapAttrsToList (
          dest: _:
          let
            relative = lib.removePrefix "/" dest;
          in
          ''
            mkdir -p "$(dirname /${lib.escapeShellArg relative})"
            "systemd-creds" decrypt \
              --tpm2-device=auto \
              --tpm2-signature="/.extra/global_credentials/tpm2-pcr-signature.json.cred" \
              --tpm2-public-key=${lib.escapeShellArg initrdPublicKeyPath} \
              --name=${lib.escapeShellArg (utils.escapeSystemdPath dest)} \
              ${encryptedSecretsRoot}/${lib.escapeShellArg relative} \
              /${lib.escapeShellArg relative}
          ''
        ) config.boot.initrd.secrets
      )}
    '';
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

    protectedSystem = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = "/run/booted-system";
      description = ''
        The system to be protected (i.e. always kept bootable on the ESP).

        By default this is the system /run/booted-system points to.
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
          --bootcounting-initial-tries ${toString cfg.bootCounting.initialTries} \
          ${secretsArgs}'';
      defaultText = lib.literalExpression ''
        ''${lib.getExe config.boot.lanzaboote.package} ''${lib.optionalString (config.boot.lanzaboote.logLevel == "debug") "-vv"} install \
          --system ''${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          --systemd ''${config.systemd.package} \
          --systemd-boot-loader-config ''${loaderConfigFile} \
          --configuration-limit ''${toString configurationLimit} \
          --allow-unsigned ''${lib.boolToString config.boot.lanzaboote.allowUnsigned} \
          --bootcounting-initial-tries ''${toString config.boot.lanzaboote.bootCounting.initialTries} \
          ''${secretsArgs}'';
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

      includeFirmwareBuiltinKeys = lib.mkEnableOption "including firmware built-in keys when enrolling the Secure Boot keys";

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

            When you combine this with automatically provisioning Secure Boot,
            you generally don't need to reboot after autoCryptenroll.
          '';
        };
      };
    };

    encryptedSecrets = {
      enable = lib.mkEnableOption "" // {
        description = ''
          Encrypt the secrets configured in {option}`boot.initrd.secrets` against the
          TPM and only append the ciphertext to the initrd.

          Secrets are encrypted at boot loader installation time with
          `systemd-creds` against a policy signed for PCR 11. The signature for each
          generation is stored on the ESP, so only images with a matching signature
          can decrypt the secrets. The initrd decrypts them back to their configured
          paths before any service that consumes them starts.

          Every entry must resolve to a regular file; directories are not supported.

          The TPM loads the public key to verify the signature, so the key must be
          RSA and no larger than the TPM's maximum key size (typically 2048 bits).
          {option}`boot.lanzaboote.encryptedSecrets.privateKeyFile` is generated
          as a 2048 bit key if it does not exist yet.

          This requires the systemd initrd with TPM2 support
          ({option}`boot.initrd.systemd.tpm2.enable`) and Secure Boot. If
          {option}`boot.lanzaboote.autoGenerateKeys.enable` is used, enable this only
          after the keys have been generated and enrolled, since Secure Boot is not
          yet active on the first boot.
        '';
      };

      privateKeyFile = lib.mkOption {
        type = lib.types.nonEmptyStr;
        default = "/var/lib/lanzaboote/secrets-tpm2-pcr-private-key.pem";
        description = ''
          Absolute path to the RSA private key used to sign the PCR 11 policy for
          the encrypted initrd secrets. The key is separate from the Secure Boot
          signing key.

          If the file does not exist at boot loader installation time, a 2048 bit
          RSA key is generated there.
        '';
      };

      publicKeyFile = lib.mkOption {
        type = lib.types.nonEmptyStr;
        default = "/var/lib/lanzaboote/secrets-tpm2-pcr-public-key.pem";
        description = ''
          Absolute path to the public key matching
          {option}`boot.lanzaboote.encryptedSecrets.privateKeyFile`. It is used for
          encryption.

          It is derived from the private key on every installation, replacing an
          existing file, so the pair cannot diverge after key rotation.
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
      {
        assertion = cfg.measuredBoot.enable -> (configurationLimit > 0 && configurationLimit <= 4);
        message = ''
          If Measured Boot is enabled, you cannot store more than 4 generations on the ESP.

            This is a strict limit introduced and enforced by systemd-pcrlock.

            Set `boot.lanzaboote.configurationLimit = 4;` to reduce the number of generations you store.
        '';
      }
    ]
    ++ lib.optionals cfg.encryptedSecrets.enable [
      {
        assertion = config.boot.initrd.systemd.tpm2.enable;
        message = ''
          Encrypted initrd secrets require TPM2 support in the initrd
          (`boot.initrd.systemd.tpm2.enable`).
        '';
      }
      {
        assertion = config.boot.initrd.systemd.tpm2.pcrphases.enable;
        message = ''
          Encrypted initrd secrets require PCR phases to be enabled
          (`boot.initrd.systemd.tpm2.pcrphases.enable`).
        '';
      }
    ];

    boot.bootspec = {
      extensions."org.nix-community.lanzaboote" = {
        sort_key = config.boot.lanzaboote.sortKey;
      }
      // lib.optionalAttrs encryptSecrets {
        encrypted_initrd_secrets = lib.getExe encryptSecretsScript;
      };
    };
    boot.loader.supportsInitrdSecrets = true;
    boot.loader.external = {
      enable = true;
      installHook = "${installHook}/bin/lzbt";
    };
    boot.initrd.systemd.services.initrd-nixos-copy-secrets = lib.mkIf encryptSecrets {
      # TPM access must be available before the secrets can be decrypted, and the
      # PCR 11 boot phase must be final so the signature matches the current PCR
      # value.
      after = [
        "tpm2.target"
        "systemd-pcrphase-initrd.service"
      ];
      # Decrypt as part of the copy service so that units ordered after it see the
      # plaintext secrets rather than the ciphertext.
      postStart = lib.getExe decryptSecretsScript;
    };
    boot.initrd.systemd.storePaths = lib.mkIf encryptSecrets [
      "${config.systemd.package}/bin/systemd-creds"
      decryptSecretsScript
    ];
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
    # Since we might want to include PCR7 (the Secure Boot policy) we can only
    # create these measurements after we have booted in a Secure Boot system
    # for the first time. Thus, run this only if Secure Boot is already enabled
    # if we autoEnrollKeys.
    systemd.services.systemd-pcrlock-secureboot-policy =
      lib.mkIf (cfg.measuredBoot.enable && cfg.autoEnrollKeys.enable)
        {
          unitConfig.ConditionSecurity = "uefi-secureboot";
        };
    systemd.services.systemd-pcrlock-secureboot-authority =
      lib.mkIf (cfg.measuredBoot.enable && cfg.autoEnrollKeys.enable)
        {
          unitConfig.ConditionSecurity = "uefi-secureboot";
        };

    systemd.services.systemd-pcrlock-make-policy = lib.mkIf cfg.measuredBoot.enable {
      wantedBy = [ "sysinit.target" ];
      # Otherwise, systemd-pcrlock will not be able to write the boot loader
      # credential to the ESP if you're not using an automounted ESP.
      after = [ "local-fs.target" ];

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
            ++ lib.optionals cfg.autoEnrollKeys.includeFirmwareBuiltinKeys [ "--firmware-builtin" ]
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
          ${installHook}/bin/lzbt
        '';
    };

    systemd.services.auto-cryptenroll = lib.mkIf cfg.measuredBoot.autoCryptenroll.enable {
      wantedBy = [ "multi-user.target" ];

      unitConfig = {
        ConditionPathExists = [
          # If this path exists the new policy was already enrolled and thus
          # does not need to be enrolled again. systemd-pcrlock will update the
          # policy in place in the same NV index of the TPM.
          "!/var/lib/auto-cryptenroll/1"
        ];
        ConditionSecurity = lib.mkIf cfg.autoEnrollKeys.enable "uefi-secureboot";
        SuccessAction = lib.mkIf cfg.measuredBoot.autoCryptenroll.autoReboot "reboot";
      };

      serviceConfig = {
        Type = "oneshot";
        # SuccessAction doesn't trigger if the service is RemainAfterExit
        RemainAfterExit = lib.mkIf (!cfg.measuredBoot.autoCryptenroll.autoReboot) true;
        StateDirectory = "auto-cryptenroll";
        ExecStart = [
          # Re-create all artifacts on the ESP to generate pcrlock measurements
          # for PCR 4. This will also create a new pcrlock policy.
          "${installHook}/bin/lzbt"
          ''
            systemd-cryptenroll \
              --wipe-slot=tpm2 \
              --tpm2-device=auto \
              --unlock-tpm2-device=auto \
              --tpm2-pcrlock=${cfg.measuredBoot.pcrlockPolicy} \
              ${cfg.measuredBoot.autoCryptenroll.device}
          ''
        ];
        ExecStartPost = "${pkgs.coreutils}/bin/touch /var/lib/auto-cryptenroll/1";
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
