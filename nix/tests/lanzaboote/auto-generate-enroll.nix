let
  pkiBundle = "/var/lib/lanzaboote-auto-generated";
  generateKeysUnit = "generate-sb-keys.service";
  prepareAutoEnrollUnit = "prepare-sb-auto-enroll.service";
in
{
  name = "lanzaboote";

  nodes.machine =
    { pkgs, ... }:
    {
      imports = [ ./common/lanzaboote.nix ];

      lanzabooteTest = {
        keyFixture = false;
        persistentRoot = true;
      };

      boot.lanzaboote = {
        inherit pkiBundle;
        autoGenerateKeys.enable = true;
        autoEnrollKeys = {
          enable = true;
        };
      };

      virtualisation.tpm.enable = true;

      environment.systemPackages = [ pkgs.sbctl ];
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      with subtest("loader.conf contains secure-boot-enroll = force"):
        loader_conf = machine.succeed("cat /boot/loader/loader.conf")
        print(loader_conf)
        t.assertIn("secure-boot-enroll force", loader_conf)

      with subtest("/nix/var/nix/profiles exists"):
        print(machine.succeed("ls -lh /nix/var/nix/profiles"))

      with subtest("sbctl.conf is written"):
        print(machine.succeed("cat /etc/sbctl/sbctl.conf"))

      with subtest("Secure Boot is not yet enabled"):
        bootctl_status = machine.succeed("bootctl status")
        print(bootctl_status)
        t.assertIn("Secure Boot: disabled", bootctl_status)

      with subtest("Secure Boot keys are auto generated"):
        machine.wait_for_unit("${generateKeysUnit}")
        print(machine.succeed("ls -lh ${pkiBundle}"))

      with subtest("Auth variables are written to ESP"):
        machine.wait_for_unit("${prepareAutoEnrollUnit}")
        print(machine.succeed("ls /boot/loader/keys/auto"))

      with subtest("Files on ESP are signed with auto generated keys from pkiBundle"):
        verify_output = machine.succeed("sbctl verify")
        print(verify_output)
        t.assertIn("✓", verify_output)

      machine.reboot()

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
    '';
}
