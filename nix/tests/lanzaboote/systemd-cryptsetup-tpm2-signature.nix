{
  name = "systemd-cryptsetup-tpm2-signature";

  nodes.machine =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      # Private keys must be in the Nix Store because the stub is installed by ./common/image.nix on the host
      privateKeyFile = pkgs.runCommand "tpm2-pcr-private-key.pem" { } ''
        ${lib.getExe pkgs.openssl} genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out
      '';
      publicKeyFile = pkgs.runCommand "tpm2-pcr-public-key.pem" { } ''
        ${lib.getExe pkgs.openssl} rsa -pubout -in ${privateKeyFile} -out $out
      '';
      privateKeyInitrdFile = pkgs.runCommand "tpm2-pcr-initrd-private-key.pem" { } ''
        ${lib.getExe pkgs.openssl} genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out
      '';
      publicKeyInitrdFile = pkgs.runCommand "tpm2-pcr-initrd-public-key.pem" { } ''
        ${lib.getExe pkgs.openssl} rsa -pubout -in ${privateKeyInitrdFile} -out $out
      '';
    in
    {
      imports = [ ./common/lanzaboote.nix ];

      virtualisation.tpm.enable = true;
      virtualisation.emptyDiskImages = [
        {
          driveConfig = {
            name = "crypt-1";
          };
          size = 32;
        }
        {
          driveConfig = {
            name = "crypt-2";
          };
          size = 32;
        }
      ];

      boot.initrd.systemd.enable = true;
      boot.initrd.luks.devices = {
        crypt-1 = {
          device = "/dev/disk/by-label/crypt-1";
          crypttabExtraOpts = [
            "tpm2-device=auto"
            "nofail"
            "headless=true"
          ];
        };
        crypt-2 = {
          device = "/dev/disk/by-label/crypt-2";
          crypttabExtraOpts = [
            "tpm2-device=auto"
            "nofail"
            "headless=true"
          ];
        };
      };
      boot.initrd.systemd.services."systemd-cryptsetup@".before = [
        "cryptsetup.target"
        "initrd-switch-root.target"
      ];

      # Measure boot phases
      boot.initrd.systemd.storePaths = [ "${config.systemd.package}/lib/systemd/systemd-pcrextend" ];
      boot.initrd.systemd.additionalUpstreamUnits = [ "systemd-pcrphase-initrd.service" ];
      boot.initrd.systemd.services.systemd-pcrphase-initrd.wantedBy = [ "initrd.target" ];
      systemd.additionalUpstreamSystemUnits = [
        "systemd-pcrphase.service"
        "systemd-pcrphase-sysinit.service"
      ];

      boot.lanzaboote.measuredBoot.pcrSignatures = [
        {
          inherit privateKeyFile;
          phases = [
            "enter-initrd"
            "enter-initrd:leave-initrd"
            "enter-initrd:leave-initrd:sysinit"
            "enter-initrd:leave-initrd:sysinit:ready"
          ];
        }
        { privateKeyFile = privateKeyInitrdFile; }
      ];
      environment.systemPackages = [ pkgs.cryptsetup ];

      systemd.tmpfiles.settings = {
        "10-tpm2-pcr-keys" = {
          "/etc/systemd/tpm2-pcr-public-key.pem".L.argument = "${publicKeyFile}";
          "/etc/systemd/tpm2-pcr-initrd-public-key.pem".L.argument = "${publicKeyInitrdFile}";
        };
      };
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      machine.wait_for_unit("default.target")

      # Setup LUKS devices
      machine.succeed("echo 1234 | cryptsetup luksFormat /dev/vdb - --label crypt-1")
      machine.succeed("echo 123456 | cryptsetup luksFormat /dev/vdc - --label crypt-2")
      # Enroll TPM2 keys
      machine.succeed("echo 1234 | systemd-cryptenroll --unlock-key-file=/dev/stdin --tpm2-device=auto --tpm2-public-key=/etc/systemd/tpm2-pcr-public-key.pem --tpm2-public-key-pcrs=11 /dev/disk/by-label/crypt-1")
      machine.succeed("echo 123456 | systemd-cryptenroll --unlock-key-file=/dev/stdin --tpm2-device=auto --tpm2-public-key=/etc/systemd/tpm2-pcr-initrd-public-key.pem --tpm2-public-key-pcrs=11 /dev/disk/by-label/crypt-2")

      # Unlock disk
      machine.succeed("systemd-cryptsetup attach crypt-1 /dev/disk/by-label/crypt-1 - tpm2-device=auto,headless=true")
      # Fail to unlock disk bound to initrd key
      machine.fail("systemd-cryptsetup attach crypt-2 /dev/disk/by-label/crypt-2 - tpm2-device=auto,headless=true")

      machine.reboot()
      machine.wait_for_unit("default.target")

      # Check for unlocked LUKS volumes
      machine.succeed("test -e /dev/mapper/crypt-1")
      machine.succeed("test -e /dev/mapper/crypt-2")
      #
    '';
}
