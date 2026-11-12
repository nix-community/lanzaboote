# With boot counting, the system that is being switched to becomes the
# `preferred` instead of the `default` entry, and systemd-boot falls back to
# the `default` entry once the preferred one has run out of tries.
{ bootCounting }:
{ lib, ... }:
{
  name = "lanzaboote-system-profiles" + (if bootCounting then "-boot-counting" else "");

  # The boot counting variant boots six times, which takes longer than the
  # default timeout allows on slow (emulated) machines.
  globalTimeout = lib.mkIf bootCounting (lib.mkForce (15 * 60));

  nodes.machine =
    {
      config,
      lib,
      extendModules,
      ...
    }:
    let
      # The system that is deployed into an extra system profile. It only
      # differs in the marker file and, with boot counting, never completes
      # booting so that its entry runs out of tries.
      customSystem =
        (extendModules {
          modules = [
            { lanzabooteTest.profileMarker = "custom"; }
          ]
          ++ lib.optional bootCounting {
            systemd.services.failing = {
              script = "exit 1";
              requiredBy = [ "boot-complete.target" ];
              before = [ "boot-complete.target" ];
              serviceConfig.Type = "oneshot";
            };
          };
        }).config.system.build.toplevel;
    in
    {
      imports = [ ./common/lanzaboote.nix ];

      options.lanzabooteTest.profileMarker = lib.mkOption {
        type = lib.types.str;
        default = "system";
        description = "Marker in /etc to tell the systems of different profiles apart.";
      };

      config = {
        lanzabooteTest.persistentRoot = true;

        boot.lanzaboote.bootCounting.initialTries = lib.mkIf bootCounting 2;

        # Boot is successful if multi-user is reached.
        systemd.targets.boot-complete.after = lib.mkIf bootCounting [ "multi-user.target" ];

        # We need this so switch-to-configuration exists.
        system.switch.enable = true;

        environment.etc."profile-marker".text = config.lanzabooteTest.profileMarker;

        # Include the profile system in the image so that it can be deployed
        # without building it inside the VM.
        system.extraDependencies = lib.mkIf (config.lanzabooteTest.profileMarker == "system") [
          customSystem
        ];
        system.build = { inherit customSystem; };
      };
    };

  testScript =
    { nodes, ... }:
    let
      system = nodes.machine.system.build.toplevel;
      inherit (nodes.machine.system.build) customSystem;
    in
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      import json

      def default_entry():
        entries = json.loads(machine.succeed("bootctl list --json=short"))
        return next(entry for entry in entries if entry.get("isDefault"))

      def marker():
        return machine.succeed("cat /etc/profile-marker").strip()

      def check_loader_conf(entry):
        loader_conf = machine.succeed("cat /boot/loader/loader.conf")
        print(loader_conf)
        key = ${if bootCounting then ''"preferred"'' else ''"default"''}
        t.assertIn(f"{key} {entry['id']}", loader_conf)

      machine.wait_for_unit("multi-user.target")
      t.assertEqual(marker(), "system")

      # The Nix store of the image is read-only, so nix-env cannot be used.
      # Create the same links that `nix-env --profile <profile> --set` would.
      def set_profile(profile, generation, toplevel):
        directory, name = profile.rsplit("/", 1)
        machine.succeed(f"mkdir -p {directory}")
        machine.succeed(f"ln -s {toplevel} {profile}-{generation}-link")
        machine.succeed(f"ln -sfn {name}-{generation}-link {profile}")

      with subtest("Deploy into an extra system profile"):
        set_profile("/nix/var/nix/profiles/system-profiles/custom", 1, "${customSystem}")
        machine.succeed("${customSystem}/bin/switch-to-configuration boot")

        print(machine.succeed("bootctl list"))
        entry = default_entry()
        t.assertIn("[custom]", entry["title"])
        t.assertTrue(entry["id"].startswith("nixos-profile-custom-generation-1-"), entry["id"])
        check_loader_conf(entry)

      with subtest("Boot into the extra system profile"):
        machine.reboot()
        machine.wait_for_unit("multi-user.target")
        t.assertEqual(marker(), "custom")
    ''
    + (
      if bootCounting then
        ''

          with subtest("Fall back to the system profile once the preferred entry is bad"):
            # The custom system never completes booting, so its second and last try fails as well.
            machine.reboot()
            machine.wait_for_unit("multi-user.target")
            t.assertEqual(marker(), "custom")
            stubs = machine.succeed("ls -1 /boot/EFI/Linux")
            print(stubs)
            t.assertRegex(stubs, r"nixos-profile-custom-generation-1-.*\+0-2\.efi")

            # systemd-boot skips the preferred entry without tries left and uses `default nixos-*`.
            machine.reboot()
            machine.wait_for_unit("multi-user.target")
            t.assertEqual(marker(), "system")
        ''
      else
        ""
    )
    + ''

      with subtest("Switch back to the system profile"):
        set_profile("/nix/var/nix/profiles/system", 2, "${system}")
        machine.succeed("${system}/bin/switch-to-configuration boot")

        print(machine.succeed("bootctl list"))
        entry = default_entry()
        t.assertNotIn("[custom]", entry["title"])
        t.assertTrue(entry["id"].startswith("nixos-generation-2-"), entry["id"])
        check_loader_conf(entry)

        machine.reboot()
        machine.wait_for_unit("multi-user.target")
        t.assertEqual(marker(), "system")

      with subtest("Garbage collect the deleted profile generation"):
        t.assertIn("nixos-profile-custom-generation-1-", machine.succeed("ls -1 /boot/EFI/Linux"))
        machine.succeed("rm /nix/var/nix/profiles/system-profiles/custom /nix/var/nix/profiles/system-profiles/custom-1-link")
        machine.succeed("/run/current-system/bin/switch-to-configuration boot")

        stubs = machine.succeed("ls -1 /boot/EFI/Linux")
        print(stubs)
        t.assertNotIn("nixos-profile-custom", stubs)
        t.assertTrue(default_entry()["id"].startswith("nixos-generation-2-"))
    '';
}
