{ pkgs
, testPkgs
, lanzabooteModule
}:

let
  inherit (pkgs) lib;

  mkSecureBootTest = { name, machine ? { }, testScript }: testPkgs.nixosTest {
    inherit name testScript;
    nodes.machine = { lib, ... }: {
      imports = [
        lanzabooteModule
        machine
      ];

      virtualisation = {
        useBootLoader = true;
        useEFIBoot = true;
        useSecureBoot = true;
      };

      boot.loader.efi = {
        canTouchEfiVariables = true;
      };
      boot.lanzaboote = {
        enable = true;
        enrollKeys = lib.mkDefault true;
        pkiBundle = ../../pki;
      };
    };
  };

  # Execute a boot test that is intended to fail.
  #
  mkUnsignedTest = { name, path, appendCrap ? false }: mkSecureBootTest {
    inherit name;
    testScript = ''
      import json
      import os.path
      bootspec = None

      def convert_to_esp(store_file_path):
          store_dir = os.path.basename(os.path.dirname(store_file_path))
          filename = os.path.basename(store_file_path)
          return f'/boot/EFI/nixos/{store_dir}-{filename}.efi'

      machine.start()
      bootspec = json.loads(machine.succeed("cat /run/current-system/boot.json")).get('v1')
      assert bootspec is not None, "Unsupported bootspec version!"
      src_path = ${path.src}
      dst_path = ${path.dst}
      machine.succeed(f"cp -rf {src_path} {dst_path}")
    '' + lib.optionalString appendCrap ''
      machine.succeed(f"echo Foo >> {dst_path}")
    '' +
    ''
      machine.succeed("sync")
      machine.crash()
      machine.start()
      machine.wait_for_console_text("Hash mismatch")
    '';
  };
in
{
  # TODO: user mode: OK
  # TODO: how to get in: {deployed, audited} mode ?
  lanzaboote-boot = mkSecureBootTest {
    name = "signed-files-boot-under-secureboot";
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };

  lanzaboote-boot-under-sd-stage1 = mkSecureBootTest {
    name = "signed-files-boot-under-secureboot-systemd-stage-1";
    machine = { ... }: {
      boot.initrd.systemd.enable = true;
    };
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };

  # So, this is the responsibility of the lanzatool install
  # to run the append-initrd-secret script
  # This test assert that lanzatool still do the right thing
  # preDeviceCommands should not have any root filesystem mounted
  # so it should not be able to find /etc/iamasecret, other than the
  # initrd's one.
  # which should exist IF lanzatool do the right thing.
  lanzaboote-with-initrd-secrets = mkSecureBootTest {
    name = "signed-files-boot-with-secrets-under-secureboot";
    machine = { ... }: {
      boot.initrd.secrets = {
        "/etc/iamasecret" = (pkgs.writeText "iamsecret" "this is a very secure secret");
      };

      boot.initrd.preDeviceCommands = ''
        grep "this is a very secure secret" /etc/iamasecret
      '';
    };
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };

  # The initrd is not directly signed. Its hash is embedded
  # into lanzaboote. To make integrity verification fail, we
  # actually have to modify the initrd. Appending crap to the
  # end is a harmless way that would make the kernel still
  # accept it.
  is-initrd-secured = mkUnsignedTest {
    name = "unsigned-initrd-do-not-boot-under-secureboot";
    path = {
      src = "bootspec.get('initrd')";
      dst = "convert_to_esp(bootspec.get('initrd'))";
    };
    appendCrap = true;
  };

  is-kernel-secured = mkUnsignedTest {
    name = "unsigned-kernel-do-not-boot-under-secureboot";
    path = {
      src = "bootspec.get('kernel')";
      dst = "convert_to_esp(bootspec.get('kernel'))";
    };
  };
  specialisation-works = mkSecureBootTest {
    name = "specialisation-still-boot-under-secureboot";
    machine = { pkgs, ... }: {
      specialisation.variant.configuration = {
        environment.systemPackages = [
          pkgs.efibootmgr
        ];
      };
    };
    testScript = ''
      machine.start()
      print(machine.succeed("ls -lah /boot/EFI/Linux"))
      print(machine.succeed("cat /run/current-system/boot.json"))
      # TODO: make it more reliable to find this filename, i.e. read it from somewhere?
      machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant.efi")
      machine.succeed("sync")
      machine.fail("efibootmgr")
      machine.crash()
      machine.start()
      print(machine.succeed("bootctl"))
      # We have efibootmgr in this specialisation.
      machine.succeed("efibootmgr")
    '';
  };
}
