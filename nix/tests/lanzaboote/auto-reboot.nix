let
  pkiBundle = "/var/lib/lanzaboote-auto-generated";
  generateKeysUnit = "generate-sb-keys.service";
  prepareAutoEnrollUnit = "prepare-sb-auto-enroll.service";
in
{
  name = "lanzaboote-auto-reboot";

  nodes.machine =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    {
      imports = [ ./common/lanzaboote.nix ];

      virtualisation.tpm.enable = true;

      lanzabooteTest = {
        keyFixture = false;
        persistentRoot = true;
      };

      boot.lanzaboote = {
        inherit pkiBundle;
        autoGenerateKeys.enable = true;
        autoEnrollKeys = {
          enable = true;
          autoReboot = true;
        };

        # Measured Boot
        configurationLimit = 8;
        measuredBoot = {
          enable = true;
          pcrs = [
            0
            1
            # PCR 2 and 3 are not consistent on aarch64.
            # 2
            # 3
            4
            7
          ];
          autoCryptenroll = {
            enable = true;
            device = config.boot.initrd.luks.devices."encrypted".device;
            autoReboot = true;
          };
        };
      };

      boot.initrd = {
        luks.devices = {
          encrypted.device = "/dev/disk/by-partlabel/encrypted";
        };

        systemd = {
          enable = true;
          repart = {
            enable = true;
          };
        };
      };

      image.repart.partitions = {
        # Use a fix type to accurately match against it with repart during
        # runtime.
        "nix-store".repartConfig.Type = lib.mkForce "9141b5b5-1a5c-4867-89f6-d3ebab0ef668";
        # Leave some padding at the end of the root partition so we have enough
        # space to create a encrypted partition.
        "root".repartConfig.PaddingMinBytes = "100M";
      };
      systemd.repart.partitions = {
        "nix-store" = {
          Type = "9141b5b5-1a5c-4867-89f6-d3ebab0ef668";
        };
        "root" = {
          Type = "root";
        };
        "encrypted" = {
          Type = "linux-generic";
          Label = "encrypted";
          Format = "ext4";
          Encrypt = "tpm2";
          SizeMaxBytes = "10M";
        };
      };

      fileSystems = {
        "/encrypted" = {
          device = "/dev/mapper/encrypted";
          fsType = config.systemd.repart.partitions."encrypted".Format;
        };
      };

      environment.systemPackages = [
        pkgs.tree
        pkgs.jq
      ];

      specialisation."new-generation".configuration = {
        environment.systemPackages = [ pkgs.efibootmgr ];
      };

      # systemd.services.systemd-pcrlock-make-policy.environment.SYSTEMD_LOG_LEVEL = "debug";
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + (import ./common/efivariables-helper.nix)
    + ''
      import json

      # First boot: provisioning Secure Boot
      machine.connect()

      # Second Boot: provisioning Measured Boot
      machine.connected = False
      machine.connect()

      # Third boot: completely enrolled system
      machine.connected = False
      machine.connect()

      with subtest("Secure Boot is enabled now"):
        bootctl_status = machine.succeed("bootctl status")
        print(bootctl_status)
        t.assertIn("Secure Boot: enabled (user)", bootctl_status)

      with subtest("Keys are not re-generated if they already exist"):
        generate_systemctl_output = machine.fail("systemctl status ${generateKeysUnit}")
        print(generate_systemctl_output)
        t.assertIn("Condition: start condition unmet", generate_systemctl_output)

      with subtest("Keys are not re-generated if they already exist"):
        prepare_systemctl_output = machine.fail("systemctl status ${prepareAutoEnrollUnit}")
        print(prepare_systemctl_output)
        t.assertIn("Condition: start condition unmet", prepare_systemctl_output)

      with subtest("pcrlock policy contains all the PCRs"):
        machine.wait_for_unit("systemd-pcrlock-make-policy.service")
        policy_json = machine.succeed("cat /var/lib/systemd/pcrlock.json | tee /dev/stderr")
        policy = json.loads(policy_json)
        pcrs = [x.get("pcr") for x in policy.get("pcrValues")]
        t.assertIn(0, pcrs)
        t.assertIn(1, pcrs)
        # t.assertIn(2, pcrs)
        # t.assertIn(3, pcrs)
        t.assertIn(4, pcrs)
        t.assertIn(7, pcrs)

      with subtest("Correct number of boots"):
        boots = machine.succeed("journalctl --list-boots --quiet | tee /dev/stderr")
        boot_count = len(boots.strip().split("\n"))
        t.assertEqual(boot_count, 3)
    '';
}
