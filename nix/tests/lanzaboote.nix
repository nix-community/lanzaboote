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
        pkiBundle = ./fixtures/uefi-keys;
      };
    };
  };

  # Execute a SB test that is expected to fail because of a hash mismatch.
  #
  # Takes a set `path` consisting of a `src` and a `dst` attribute. The file at
  # `src` is copied to `dst` inside th VM. Optionally append some random data
  # ("crap") to the end of the file at `dst`. This is useful to easily change
  # the hash of a file and produce a hash mismatch when booting the stub.
  mkHashMismatchTest = { name, path, appendCrap ? false }: mkSecureBootTest {
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
  basic = mkSecureBootTest {
    name = "lanzaboote";
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };

  systemd-initrd = mkSecureBootTest {
    name = "lanzaboote-systemd-initrd";
    machine = { ... }: {
      boot.initrd.systemd.enable = true;
    };
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };

  # Test that a secret is appended to the initrd during installation.
  # 
  # During the execution of `preDeviceCommands`, no filesystem should be
  # mounted. The only place to find `/etc/iamasecret` then, is in the initrd.
  initrd-secrets = mkSecureBootTest {
    name = "lanzaboote-initrd-secrets";
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
  # into the UKI. To make integrity verification fail, we
  # actually have to modify the initrd. Appending crap to the
  # end is a harmless way that would make the kernel still
  # accept it.
  secured-initrd = mkHashMismatchTest {
    name = "lanzaboote-secured-initrd";
    path = {
      src = "bootspec.get('initrd')";
      dst = "convert_to_esp(bootspec.get('initrd'))";
    };
    appendCrap = true;
  };

  secured-kernel = mkHashMismatchTest {
    name = "lanzaboote-secured-kernel";
    path = {
      src = "bootspec.get('kernel')";
      dst = "convert_to_esp(bootspec.get('kernel'))";
    };
    appendCrap = true;
  };

  specialisation = mkSecureBootTest {
    name = "lanzaboote-specialisation";
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
      # TODO: make it more reliable to find this filename, i.e. read it from somewhere?
      machine.succeed("bootctl set-default nixos-generation-1-specialisation-variant.efi")
      machine.succeed("sync")
      machine.fail("efibootmgr")
      machine.crash()
      machine.start()
      print(machine.succeed("bootctl"))
      # Only the specialisation contains the efibootmgr binary.
      machine.succeed("efibootmgr")
    '';
  };

  systemd-boot-loader-config = mkSecureBootTest {
    name = "lanzaboote-systemd-boot-loader-config";
    machine = {
      boot.loader.timeout = 0;
      boot.loader.systemd-boot.consoleMode = "auto";
    };
    testScript = ''
      machine.start()

      actual_loader_config = machine.succeed("cat /boot/loader/loader.conf")
      expected_loader_config = "timeout 0\nconsole-mode auto\n"
      
      assert actual_loader_config == expected_loader_config, \
        f"Actual: '{actual_loader_config}' is not equal to expected: '{expected_loader_config}'"
    '';
  };
}
