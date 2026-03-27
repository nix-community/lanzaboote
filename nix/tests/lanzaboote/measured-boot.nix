{
  name = "lanzaboote-measured-boot";

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
        persistentRoot = true;
      };

      boot.lanzaboote = {
        configurationLimit = 8;
        measuredBoot = {
          enable = true;
          pcrs = [
            0
            1
            2
            3
            4
            7
          ];
          autoCryptenroll = {
            enable = true;
            device = config.boot.initrd.luks.devices."encrypted".device;
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
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + (import ./common/efivariables-helper.nix)
    + ''
      import json

      machine.start()

      with subtest("TPM2 is setup"):
        machine.wait_for_unit("systemd-tpm2-setup.service")

      with subtest("vendor pcrlock components are present in /etc"):
        print(machine.succeed("tree /etc/pcrlock.d"))

      with subtest("pcrlock measurements are generated"):
        machine.wait_for_unit("systemd-pcrlock-firmware-code.service")
        machine.wait_for_unit("systemd-pcrlock-secureboot-authority.service")
        machine.wait_for_unit("systemd-pcrlock-secureboot-policy.service")

        with subtest("ESP artifact measurements are generated"):
          machine.wait_for_unit("prepare-auto-cryptenroll.service")
          print(machine.succeed("tree /var/lib/pcrlock.d"))
          print(machine.succeed("stat /var/lib/pcrlock.d/630-bootloader.pcrlock.d/current.pcrlock"))
          print(machine.succeed("stat /var/lib/pcrlock.d/635-lanzaboote.pcrlock.d/1.pcrlock"))
          print(machine.succeed("stat /var/lib/pcrlock.d/635-lanzaboote.pcrlock.d/1-new-generation.pcrlock"))

      with subtest("pcrlock policy is generated"):
        machine.wait_for_unit("systemd-pcrlock-make-policy.service")
        policy_json = machine.succeed("cat /var/lib/systemd/pcrlock.json | tee /dev/stderr")

        with subtest("pcrlock policy contains static PCRs"):
          policy = json.loads(policy_json)
          pcrs = [x.get("pcr") for x in policy.get("pcrValues")]
          t.assertIn(0, pcrs)
          t.assertIn(1, pcrs)
          t.assertIn(2, pcrs)
          t.assertIn(3, pcrs)
          t.assertIn(7, pcrs)

        with subtest("pcrlock policy doesn't contain dynamic PCRs yet"):
          # Doesn't contain PCR 4 yet because the currently booted Lanzaboote
          # image was not measured. Only after a reboot will we boot the correct
          # Lanzaboote image.
          t.assertNotIn(4, pcrs)

      with subtest("Encrypted partition is available and mounted"):
        print(machine.succeed("findmnt /encrypted"))
        print(machine.succeed("dmsetup info /dev/mapper/encrypted"))

      with subtest("New policy is automatically enrolled"):
        machine.wait_for_unit("auto-cryptenroll.service")
        metadata_json = machine.succeed("cryptsetup luksDump /dev/disk/by-partlabel/encrypted --dump-json-metadata | tee /dev/stderr")
        metadata = json.loads(metadata_json)

        with subtest("Slot 0 was wiped after it was initially populated by systemd-repart"):
          t.assertFalse(metadata.get("tokens").get("0"))
        with subtest("Slot 1 is populated with pcrlock policy"):
          t.assertTrue(metadata.get("tokens").get("1").get("tpm2_pcrlock"))

      with subtest("systemd-pcrlock log works"):
        print(machine.succeed("/run/current-system/systemd/lib/systemd/systemd-pcrlock log"))

      # This will only work if all the generations are re-generated (and thus
      # re-measured) and a new policy is enrolled via systemd-pcrlock.
      with subtest("Reboot the system"):
        machine.reboot()

      with subtest("pcrlock policy contains all the PCRs"):
        machine.wait_for_unit("systemd-pcrlock-make-policy.service")
        policy_json = machine.succeed("cat /var/lib/systemd/pcrlock.json | tee /dev/stderr")
        policy = json.loads(policy_json)
        pcrs = [x.get("pcr") for x in policy.get("pcrValues")]
        t.assertIn(0, pcrs)
        t.assertIn(1, pcrs)
        t.assertIn(2, pcrs)
        t.assertIn(3, pcrs)
        t.assertIn(4, pcrs)
        t.assertIn(7, pcrs)
    '';
}
