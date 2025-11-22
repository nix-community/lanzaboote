{
  config,
  lib,
  pkgs,
  modulesPath,
  ...
}:
let
  espLabel = "esp";
  nixStorePartitionLabel = "nix-store";

  cfg = config.boot.lanzaboote;

  authVariables =
    let
      format = pkgs.formats.yaml { };
      sbctlConfig = format.generate "sbctl.conf" {
        keydir = "${../../fixtures/uefi-keys/keys}";
        guid = "${../../fixtures/uefi-keys/GUID}";
      };
    in
    pkgs.runCommand "auth-variables" { nativeBuildInputs = [ pkgs.sbctl ]; } ''
      mkdir -p $out
      cd $out
      sbctl enroll-keys \
        --yes-this-might-brick-my-machine \
        --config ${sbctlConfig} --export auth
    '';

  espFiles = pkgs.runCommand "esp-files" { } (
    ''
      mkdir -p $out
      ln -s ${config.system.build.toplevel} system-1-link
      ${cfg.installCommand} \
    ''
    + (
      if config.lanzabooteTest.keyFixture then
        # Use the key fixtures directly because we cannot set them via the module
        # as the options cannot point to store paths.
        ''
          --public-key ${../../fixtures/uefi-keys}/keys/db/db.pem \
          --private-key ${../../fixtures/uefi-keys}/keys/db/db.key \
        ''
      else
        ''
          --public-key ${cfg.publicKeyFile} \
          --private-key ${cfg.privateKeyFile} \
        ''
    )
    + ''
      $out \
      system-1-link
    ''
  );
in
{
  imports = [ "${modulesPath}/image/repart.nix" ];

  fileSystems = {
    "/boot" = {
      device = "/dev/disk/by-partlabel/${espLabel}";
      fsType = "vfat";
      options = [ "umask=077" ];
    };
    "/" = {
      fsType = "tmpfs";
      options = [ "mode=755" ];
    };
    "/nix/store" = {
      device = "/dev/disk/by-partlabel/${nixStorePartitionLabel}";
      fsType = "erofs";
      options = [ "ro" ];
    };
  };

  system.image = {
    id = config.system.name;
    version = config.system.nixos.version;
  };

  system.build.espFiles = espFiles;

  image.repart = {
    name = config.system.name;
    mkfsOptions = {
      # Makes the build significantly faster by slimming down the images.
      erofs = [ "-z lz4" ];
    };
    partitions = {
      "esp" = {
        contents = {
          "/".source = espFiles;
        }
        // lib.optionalAttrs config.lanzabooteTest.keyFixture {
          "/loader/keys/auto/PK.auth".source = "${authVariables}/PK.auth";
          "/loader/keys/auto/KEK.auth".source = "${authVariables}/KEK.auth";
          "/loader/keys/auto/db.auth".source = "${authVariables}/db.auth";
        };
        repartConfig = {
          Type = "esp";
          Format = config.fileSystems."/boot".fsType;
          Label = espLabel;
          SizeMinBytes = if config.nixpkgs.hostPlatform.isx86_64 then "64M" else "96M";
          UUID = "a3c9c5a1-1a9a-451c-bdac-a80bacb4170b";
        };
      };
      "nix-store" = {
        storePaths = [ config.system.build.toplevel ];
        nixStorePrefix = "/";
        repartConfig = {
          Type = "linux-generic";
          Format = config.fileSystems."/nix/store".fsType;
          Label = nixStorePartitionLabel;
          Minimize = "best";
        };
      };
    };
  };

  virtualisation = {
    directBoot.enable = false;
    mountHostNixStore = false;
    fileSystems = lib.mkForce { };
    useDefaultFilesystems = false;

    useEFIBoot = true;

    useSecureBoot = true;
    efi.OVMF = pkgs.OVMFFull.fd;
  };
}
