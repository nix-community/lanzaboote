let
  sortKey = "mySpecialSortKey";
  sortKeySpectator = "spectatorSortKey";
  sortKeyBad = "allthewayontop";
in
{
  name = "lanzaboote boot counting";

  nodes = {
    machine = {
      imports = [ ./common/lanzaboote.nix ];

      boot.lanzaboote = {
        inherit sortKey;
        bootCounting.initialTries = 2;
      };

      # Boot is successful if multi-user is reached
      systemd.targets.boot-complete.after = [ "multi-user.target" ];

      specialisation = {
        # This specialisation is ordered first, but it will fail to boot successfully
        bad.configuration =
          { lib, ... }:
          {
            boot.lanzaboote.sortKey = lib.mkForce sortKeyBad;

            systemd.services."failing" = {
              script = "exit 1";
              requiredBy = [ "boot-complete.target" ];
              before = [ "boot-complete.target" ];
              serviceConfig.Type = "oneshot";
            };
          };

        # This specialisation should be ordered at the bottom, and we should never boot it
        spectator.configuration =
          { lib, ... }:
          {
            boot.lanzaboote.sortKey = lib.mkForce sortKeySpectator;
          };
        # Specialisation with boot counting turned off, this should not matter
        spectator2.configuration =
          { lib, ... }:
          {
            boot.lanzaboote = {
              sortKey = lib.mkForce sortKeySpectator;
              bootCounting.initialTries = lib.mkForce 0;
            };
          };
      };
    };
  };

  testScript =
    { nodes, ... }:
    let
      orig = nodes.machine.system.build.toplevel;
      bad = nodes.machine.specialisation.bad.configuration.system.build.toplevel;
    in
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    +
      # python
      ''
        orig = "${orig}"
        bad = "${bad}"

        def check_current_system(system_path:str):
          current_sys = machine.succeed('readlink -f /run/current-system').strip()
          print(f'current system: {current_sys}')
          machine.succeed(f'test "{current_sys}" = "{system_path}"')

        def check_boot_entry(
          generation_counter:int,
          specialisation:str|None,
          boot_counter:int=0,
          bad_counter:int=0
        ):
          regex = rf"^/boot/EFI/Linux/nixos-generation-{generation_counter}"

          if specialisation:
            regex += f"-specialisation-{specialisation}"

          regex += r"-[0-9a-z]{52}"

          if boot_counter != 0 or bad_counter != 0:
            regex += rf"\+{boot_counter}"
            if bad_counter != 0:
              regex += rf"-{bad_counter}"

          regex += r"\.efi$"

          find_command = rf"find /boot/EFI/Linux/ -regextype posix-extended -regex '{regex}' -type f | grep -q '.'"

          machine.succeed(find_command)

        machine.start()
        machine.wait_for_unit("multi-user.target")
        # Ensure we booted using an entry with counters enabled
        machine.succeed(
          "test -e /sys/firmware/efi/efivars/LoaderBootCountPath-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
        )
        print(machine.succeed("bootctl list"))
        check_current_system(bad)
        check_boot_entry(generation_counter=1, specialisation=None, boot_counter=2)
        check_boot_entry(generation_counter=1, specialisation="spectator", boot_counter=2)
        check_boot_entry(generation_counter=1, specialisation="bad", boot_counter=1, bad_counter=1)
        machine.shutdown()

        machine.start()
        machine.wait_for_unit("multi-user.target")
        print(machine.succeed("bootctl list"))
        check_current_system(bad)
        check_boot_entry(generation_counter=1, specialisation=None, boot_counter=2)
        check_boot_entry(generation_counter=1, specialisation="spectator", boot_counter=2)
        check_boot_entry(generation_counter=1, specialisation="bad", boot_counter=0, bad_counter=2)
        machine.shutdown()

        # Should boot back into original configuration
        machine.start()
        check_current_system(orig)
        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("systemd-bless-boot.service")
        print(machine.succeed("bootctl list"))
        check_boot_entry(generation_counter=1, specialisation=None)
        check_boot_entry(generation_counter=1, specialisation="spectator", boot_counter=2)
        check_boot_entry(generation_counter=1, specialisation="bad", boot_counter=0, bad_counter=2)
        machine.shutdown()
      '';
}
