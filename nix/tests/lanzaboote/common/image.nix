{
  config,
  lib,
  pkgs,
  modulesPath,
  ...
}:
let
  rootPartitionLabel = "root";

  authVariables =
    let
      format = pkgs.formats.yaml { };
      sbctlConfig = format.generate "sbctl.conf" {
        keydir = "${../../fixtures/uefi-keys/keys}";
        guid = "${../../fixtures/uefi-keys/GUID}";
      };
    in
    pkgs.runCommandNoCC "auth-variables" { nativeBuildInputs = [ pkgs.sbctl ]; } ''
      mkdir -p $out
      cd $out
      sbctl enroll-keys \
        --yes-this-might-brick-my-machine \
        --config ${sbctlConfig} --export auth
    '';

  espFiles = pkgs.runCommand "esp-files" { } ''
    mkdir -p $out
    ln -s ${config.system.build.toplevel} system-1-link

    ${config.boot.lanzaboote.installCommand} \
      $out \
      system-1-link
  '';
in
{
  imports = [ "${modulesPath}/image/repart.nix" ];

  fileSystems = {
    "/" = {
      device = "/dev/disk/by-partlabel/${rootPartitionLabel}";
      fsType = "ext4";
    };
  };

  system.image = {
    id = config.system.name;
    version = config.system.nixos.version;
  };

  system.build.espFiles = espFiles;

  image.repart = {
    name = config.system.name;
    partitions = {
      "esp" = {
        contents = {
          "/".source = espFiles;
        }
        // lib.optionalAttrs config.virtualisation.useSecureBoot {
          "/loader/keys/auto/PK.auth".source = "${authVariables}/PK.auth";
          "/loader/keys/auto/KEK.auth".source = "${authVariables}/KEK.auth";
          "/loader/keys/auto/db.auth".source = "${authVariables}/db.auth";
        };
        repartConfig = {
          Type = "esp";
          Format = "vfat";
          SizeMinBytes = if config.nixpkgs.hostPlatform.isx86_64 then "64M" else "96M";
          UUID = "a3c9c5a1-1a9a-451c-bdac-a80bacb4170b";
        };
      };
      "root" = {
        storePaths = [ config.system.build.toplevel ];
        repartConfig = {
          Type = "root";
          Format = config.fileSystems."/".fsType;
          Label = rootPartitionLabel;
          Minimize = "guess";
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
