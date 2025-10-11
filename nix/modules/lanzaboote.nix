{ lib, config, options, pkgs, ... }:
let
  cfg = config.boot.lanzaboote;

  sbctlWithPki = pkgs.sbctl.override {
    databasePath = "/tmp/pki";
  };

  loaderSettingsFormat = pkgs.formats.keyValue {
    mkKeyValue = k: v: if v == null then "" else
    lib.generators.mkKeyValueDefault { } " " k v;
  };

  loaderConfigFile = loaderSettingsFormat.generate "loader.conf" cfg.settings;

  configurationLimit = if cfg.configurationLimit == null then 0 else cfg.configurationLimit;

  # Build a directory with all the extra rEFInd files
  refindExtraFilesDir = pkgs.runCommand "refind-extra-files" { } ''
    mkdir -p $out
    ${lib.concatStringsSep "\n" (lib.mapAttrsToList (dest: src: ''
      mkdir -p $out/$(dirname "${dest}")
      cp -r ${src} $out/${dest}
    '') cfg.refind.extraFiles)}
  '';

  # Generate extra config file
  refindExtraConfigFile = pkgs.writeText "refind-extra.conf" cfg.refind.extraConfig;

  # Utility function to fetch rEFInd themes from GitHub
  fetchRefindTheme =
    { owner
    , repo
    , rev
    , sha256
    , themeName ? repo
    , themeSubdir ? null
    }:
    let
      src = pkgs.fetchFromGitHub {
        inherit owner repo rev sha256;
      };
      themeDir = if themeSubdir != null then "${src}/${themeSubdir}" else src;
    in
    lib.mapAttrs'
      (name: type:
      lib.nameValuePair
        "themes/${themeName}/${name}"
        "${themeDir}/${name}"
      )
      (builtins.readDir themeDir);
in
{
  options.boot.lanzaboote = {
    enable = lib.mkEnableOption "Enable the LANZABOOTE";

    enrollKeys = lib.mkEnableOption "Do not use this option. Only for used for integration tests! Automatic enrollment of the keys using sbctl";

    bootloader = lib.mkOption {
      type = lib.types.enum [ "systemd-boot" "refind" ];
      default = "systemd-boot";
      description = ''
        Which bootloader to use with Lanzaboote.

        - systemd-boot: Simple UEFI boot manager (default)
        - refind: Graphical UEFI boot manager with advanced features
      '';
    };

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
      type = lib.types.nullOr lib.types.path;
      description = "PKI bundle containing db, PK, KEK";
    };

    publicKeyFile = lib.mkOption {
      type = lib.types.path;
      default = "${cfg.pkiBundle}/keys/db/db.pem";
      defaultText = "\${cfg.pkiBundle}/keys/db/db.pem";
      description = "Public key to sign your boot files";
    };

    privateKeyFile = lib.mkOption {
      type = lib.types.path;
      default = "${cfg.pkiBundle}/keys/db/db.key";
      defaultText = "\${cfg.pkiBundle}/keys/db/db.key";
      description = "Private key to sign your boot files";
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = if cfg.bootloader == "refind" then pkgs.lzbt-refind else pkgs.lzbt;
      defaultText = "pkgs.lzbt or pkgs.lzbt-refind depending on bootloader choice";
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
      };

      defaultText = ''
        {
          timeout = config.boot.loader.timeout;
          console-mode = config.boot.loader.systemd-boot.consoleMode;
          editor = config.boot.loader.systemd-boot.editor;
          default = "nixos-*";
        }
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

    refind = {
      package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.refind;
        defaultText = "pkgs.refind";
        description = "rEFInd package to use";
      };

      configTemplate = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Optional custom rEFInd configuration template.
          This will be appended to the auto-generated configuration.
        '';
      };

      extraConfig = lib.mkOption {
        type = lib.types.lines;
        default = "";
        example = ''
          # Custom rEFInd settings
          resolution 1920 1080
          use_graphics_for linux
          scanfor manual
        '';
        description = ''
          Additional rEFInd configuration to append to the generated config.
          This is appended after the template (if provided) and before the
          auto-generated NixOS boot entries.
        '';
      };

      extraFiles = lib.mkOption {
        type = lib.types.attrsOf lib.types.path;
        default = { };
        example = lib.literalExpression ''
          {
            "themes/my-theme/theme.conf" = ./my-theme/theme.conf;
            "themes/my-theme/background.png" = ./my-theme/background.png;
            "themes/my-theme/icons/os_nixos.png" = ./my-theme/icons/os_nixos.png;
          }
        '';
        description = ''
          Additional files to install to the rEFInd directory on the ESP.
          The attribute name is the relative path under EFI/refind/,
          and the value is the source file path.

          This is useful for installing themes, custom icons, or drivers.

          Example for a complete theme:
          ```nix
          extraFiles = {
            "themes/my-theme/theme.conf" = ./my-theme/theme.conf;
            "themes/my-theme/background.png" = ./my-theme/background.png;
            "themes/my-theme/icons/os_nixos.png" = ./my-theme/icons/os_nixos.png;
          };

          extraConfig = '''
            include themes/my-theme/theme.conf
          ''';
          ```
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    boot.bootspec = {
      enable = true;
      extensions."org.nix-community.lanzaboote" = {
        sort_key = config.boot.lanzaboote.sortKey;
      };
    };
    boot.loader.supportsInitrdSecrets = true;
    boot.loader.external = {
      enable = true;
      installHook = pkgs.writeShellScript "bootinstall" ''
        ${lib.optionalString cfg.enrollKeys ''
          ${lib.getExe' pkgs.coreutils "mkdir"} -p /tmp/pki
          ${lib.getExe' pkgs.coreutils "cp"} -r ${cfg.pkiBundle}/* /tmp/pki
          ${lib.getExe sbctlWithPki} enroll-keys --yes-this-might-brick-my-machine
        ''}

        # Use the system from the kernel's hostPlatform because this should
        # always, even in the cross compilation case, be the right system.
        ${lib.getExe cfg.package} install \
          --system ${config.boot.kernelPackages.stdenv.hostPlatform.system} \
          ${lib.optionalString (cfg.bootloader == "systemd-boot") ''
            --systemd ${config.systemd.package} \
            --systemd-boot-loader-config ${loaderConfigFile} \
          ''} \
          ${lib.optionalString (cfg.bootloader == "refind") ''
            --refind ${cfg.refind.package} \
            ${lib.optionalString (cfg.refind.configTemplate != null) "--refind-config-template ${cfg.refind.configTemplate}"} \
            ${lib.optionalString (cfg.refind.extraConfig != "") "--refind-extra-config ${refindExtraConfigFile}"} \
            ${lib.optionalString (cfg.refind.extraFiles != {}) "--refind-extra-files ${refindExtraFilesDir}"} \
          ''} \
          --public-key ${cfg.publicKeyFile} \
          --private-key ${cfg.privateKeyFile} \
          --configuration-limit ${toString configurationLimit} \
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
        ${lib.getExe' pkgs.sbsigntool "sbsign"} --key '${cfg.privateKeyFile}' --cert '${cfg.publicKeyFile}' /run/fwupd-efi/fwupd*.efi
      '';
    };

    services.fwupd.uefiCapsuleSettings = lib.mkIf config.services.fwupd.enable {
      DisableShimForSecureBoot = true;
    };
  };

  # Export utilities for use in configurations
  _module.args.lanzaboote-utils = {
    inherit fetchRefindTheme;
  };
}
